use rizz_v::executor::RunConfig;
use rizz_v::{ResultSpec, parse_register_assignment, run_pipeline};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::str::FromStr;

const DEFAULT_MAX_STEPS: u64 = 10_000;
const DEFAULT_PORT: u16 = 3000;
const INDEX_HTML: &str = include_str!("../viz3.html");

enum Command {
    Cli(CliArgs),
    Serve(ServeArgs),
}

struct CliArgs {
    input: PathBuf,
    asm_out: PathBuf,
    trace_out: PathBuf,
    analysis_out: PathBuf,
    result_out: PathBuf,
    max_steps: u64,
    run_config: RunConfig,
    result_spec: ResultSpec,
}

struct ServeArgs {
    port: u16,
}

#[derive(Debug, Deserialize)]
struct ApiRunRequest {
    source: String,
    max_steps: Option<u64>,
    register_inputs: Vec<ApiRegisterInput>,
    result: Option<ApiResultRequest>,
}

#[derive(Debug, Deserialize)]
struct ApiRegisterInput {
    register: String,
    value: i64,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ApiResultRequest {
    Scalar {
        register: Option<String>,
    },
    Array {
        ptr_register: Option<String>,
        length_register: Option<String>,
        elem_width: Option<u8>,
        signed: Option<bool>,
    },
}

#[derive(Debug, Serialize)]
struct ApiError {
    error: String,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    match parse_args(env::args().skip(1))? {
        Command::Cli(args) => run_cli(args),
        Command::Serve(args) => run_server(args),
    }
}

fn run_cli(args: CliArgs) -> Result<(), String> {
    let source = fs::read_to_string(&args.input)
        .map_err(|err| format!("failed to read {}: {err}", args.input.display()))?;
    let artifacts = run_pipeline(&source, &args.run_config, &args.result_spec, args.max_steps)?;

    write_json(
        &args.asm_out,
        &serde_json::to_string_pretty(&artifacts.assembly).map_err(|err| err.to_string())?,
    )?;
    write_json(
        &args.trace_out,
        &serde_json::to_string_pretty(&artifacts.trace).map_err(|err| err.to_string())?,
    )?;
    write_json(
        &args.analysis_out,
        &serde_json::to_string_pretty(&artifacts.analysis).map_err(|err| err.to_string())?,
    )?;
    write_json(
        &args.result_out,
        &serde_json::to_string_pretty(&artifacts.result).map_err(|err| err.to_string())?,
    )?;

    println!("assembly: {}", args.asm_out.display());
    println!("trace: {}", args.trace_out.display());
    println!("analysis: {}", args.analysis_out.display());
    println!("result: {}", args.result_out.display());
    Ok(())
}

fn run_server(args: ServeArgs) -> Result<(), String> {
    let listener = TcpListener::bind(("127.0.0.1", args.port))
        .map_err(|err| format!("failed to bind server on port {}: {err}", args.port))?;
    println!("server: http://127.0.0.1:{}", args.port);
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(err) = handle_connection(stream) {
                    eprintln!("request failed: {err}");
                }
            }
            Err(err) => eprintln!("connection failed: {err}"),
        }
    }
    Ok(())
}

fn handle_connection(mut stream: TcpStream) -> Result<(), String> {
    let (method, path, body) = read_request(&mut stream)?;
    match (method.as_str(), path.as_str()) {
        ("GET", "/") | ("GET", "/index.html") => write_response(
            &mut stream,
            "200 OK",
            "text/html; charset=utf-8",
            INDEX_HTML.as_bytes(),
        ),
        ("POST", "/api/run") => {
            let request: ApiRunRequest = serde_json::from_slice(&body)
                .map_err(|err| format!("invalid JSON request: {err}"))?;
            let run_config = RunConfig {
                input_registers: request
                    .register_inputs
                    .into_iter()
                    .map(|input| {
                        parse_register_assignment(&format!("{}={}", input.register, input.value))
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            };
            let result_spec = parse_api_result_request(request.result)?;

            match run_pipeline(
                &request.source,
                &run_config,
                &result_spec,
                request.max_steps.unwrap_or(DEFAULT_MAX_STEPS),
            ) {
                Ok(response) => {
                    let json = serde_json::to_vec(&response)
                        .map_err(|err| format!("serialize failed: {err}"))?;
                    write_response(&mut stream, "200 OK", "application/json", &json)
                }
                Err(err) => {
                    let json = serde_json::to_vec(&ApiError { error: err })
                        .map_err(|serr| format!("serialize error failed: {serr}"))?;
                    write_response(&mut stream, "400 Bad Request", "application/json", &json)
                }
            }
        }
        _ => write_response(
            &mut stream,
            "404 Not Found",
            "application/json",
            br#"{"error":"not found"}"#,
        ),
    }
}

fn read_request(stream: &mut TcpStream) -> Result<(String, String, Vec<u8>), String> {
    let mut buffer = Vec::new();
    let mut temp = [0u8; 4096];
    let mut headers_end = None;
    let mut content_length = 0usize;

    loop {
        let bytes_read = stream
            .read(&mut temp)
            .map_err(|err| format!("failed to read request: {err}"))?;
        if bytes_read == 0 {
            break;
        }
        buffer.extend_from_slice(&temp[..bytes_read]);

        if headers_end.is_none() {
            headers_end = find_header_end(&buffer);
            if let Some(end) = headers_end {
                content_length = parse_content_length(&buffer[..end])?;
            }
        }

        if let Some(end) = headers_end
            && buffer.len() >= end + content_length
        {
            break;
        }
    }

    let header_end = headers_end.ok_or_else(|| "malformed HTTP request".to_string())?;
    let request_line_end = buffer[..header_end]
        .windows(2)
        .position(|window| window == b"\r\n")
        .ok_or_else(|| "missing request line".to_string())?;
    let request_line = std::str::from_utf8(&buffer[..request_line_end])
        .map_err(|err| format!("invalid request line: {err}"))?;
    let mut parts = request_line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| "missing request method".to_string())?
        .to_string();
    let path = parts
        .next()
        .ok_or_else(|| "missing request path".to_string())?
        .to_string();
    let body = buffer[header_end..].to_vec();
    Ok((method, path, body))
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|index| index + 4)
}

fn parse_content_length(headers: &[u8]) -> Result<usize, String> {
    let headers = std::str::from_utf8(headers).map_err(|err| format!("invalid headers: {err}"))?;
    for line in headers.lines() {
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            return value
                .trim()
                .parse::<usize>()
                .map_err(|err| format!("invalid content-length: {err}"));
        }
    }
    Ok(0)
}

fn write_response(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &[u8],
) -> Result<(), String> {
    let headers = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream
        .write_all(headers.as_bytes())
        .and_then(|_| stream.write_all(body))
        .map_err(|err| format!("failed to write response: {err}"))
}

fn write_json(path: &Path, contents: &str) -> Result<(), String> {
    fs::write(path, contents).map_err(|err| format!("failed to write {}: {err}", path.display()))
}

fn parse_result_register(name: &str) -> Result<rizz_v::reg::Register, String> {
    rizz_v::reg::Register::from_str(name).map_err(|_| format!("unknown result register: {name}"))
}

fn parse_api_result_request(request: Option<ApiResultRequest>) -> Result<ResultSpec, String> {
    match request {
        None => Ok(ResultSpec::default()),
        Some(ApiResultRequest::Scalar { register }) => Ok(ResultSpec::Scalar {
            register: parse_result_register(register.as_deref().unwrap_or("a0"))?,
        }),
        Some(ApiResultRequest::Array {
            ptr_register,
            length_register,
            elem_width,
            signed,
        }) => Ok(ResultSpec::Array {
            ptr_register: parse_result_register(ptr_register.as_deref().unwrap_or("a0"))?,
            length_register: parse_result_register(length_register.as_deref().unwrap_or("a1"))?,
            elem_width: elem_width.unwrap_or(4),
            signed: signed.unwrap_or(true),
        }),
    }
}

fn parse_args<I>(mut args: I) -> Result<Command, String>
where
    I: Iterator<Item = String>,
{
    let first = args.next().ok_or_else(usage)?;
    if first == "serve" {
        let mut port = DEFAULT_PORT;
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--port" => {
                    let raw = args.next().ok_or_else(usage)?;
                    port = raw
                        .parse()
                        .map_err(|_| format!("invalid --port value: {raw}"))?;
                }
                "--help" | "-h" => return Err(usage()),
                _ => return Err(format!("unknown argument: {arg}\n{}", usage())),
            }
        }
        return Ok(Command::Serve(ServeArgs { port }));
    }

    let input_path = PathBuf::from(&first);
    let stem = input_path
        .file_stem()
        .and_then(|value| value.to_str())
        .map(str::to_owned)
        .ok_or_else(|| format!("invalid input file name: {first}"))?;

    let mut parsed = CliArgs {
        input: input_path,
        asm_out: PathBuf::from(format!("{stem}.asm.json")),
        trace_out: PathBuf::from(format!("{stem}.trace.json")),
        analysis_out: PathBuf::from(format!("{stem}.analysis.json")),
        result_out: PathBuf::from(format!("{stem}.result.json")),
        max_steps: DEFAULT_MAX_STEPS,
        run_config: RunConfig::default(),
        result_spec: ResultSpec::default(),
    };

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--asm-out" => parsed.asm_out = PathBuf::from(args.next().ok_or_else(usage)?),
            "--trace-out" => parsed.trace_out = PathBuf::from(args.next().ok_or_else(usage)?),
            "--analysis-out" => parsed.analysis_out = PathBuf::from(args.next().ok_or_else(usage)?),
            "--result-out" => parsed.result_out = PathBuf::from(args.next().ok_or_else(usage)?),
            "--max-steps" => {
                let raw = args.next().ok_or_else(usage)?;
                parsed.max_steps = raw
                    .parse()
                    .map_err(|_| format!("invalid --max-steps value: {raw}"))?;
            }
            "--reg" => {
                let raw = args.next().ok_or_else(usage)?;
                parsed
                    .run_config
                    .input_registers
                    .push(parse_register_assignment(&raw)?);
            }
            "--result-kind" => {
                let raw = args.next().ok_or_else(usage)?;
                parsed.result_spec = match raw.as_str() {
                    "scalar" => ResultSpec::Scalar {
                        register: match parsed.result_spec.clone() {
                            ResultSpec::Scalar { register } => register,
                            _ => rizz_v::reg::Register::A0,
                        },
                    },
                    "array" => ResultSpec::Array {
                        ptr_register: match parsed.result_spec.clone() {
                            ResultSpec::Array { ptr_register, .. } => ptr_register,
                            _ => rizz_v::reg::Register::A0,
                        },
                        length_register: match parsed.result_spec.clone() {
                            ResultSpec::Array {
                                length_register, ..
                            } => length_register,
                            _ => rizz_v::reg::Register::A1,
                        },
                        elem_width: match parsed.result_spec.clone() {
                            ResultSpec::Array { elem_width, .. } => elem_width,
                            _ => 4,
                        },
                        signed: match parsed.result_spec.clone() {
                            ResultSpec::Array { signed, .. } => signed,
                            _ => true,
                        },
                    },
                    _ => return Err(format!("invalid --result-kind value: {raw}")),
                };
            }
            "--result-register" => {
                let register = parse_result_register(&args.next().ok_or_else(usage)?)?;
                parsed.result_spec = ResultSpec::Scalar { register };
            }
            "--result-ptr-register" => {
                let ptr_register = parse_result_register(&args.next().ok_or_else(usage)?)?;
                let (length_register, elem_width, signed) = match parsed.result_spec.clone() {
                    ResultSpec::Array {
                        length_register,
                        elem_width,
                        signed,
                        ..
                    } => (length_register, elem_width, signed),
                    _ => (rizz_v::reg::Register::A1, 4, true),
                };
                parsed.result_spec = ResultSpec::Array {
                    ptr_register,
                    length_register,
                    elem_width,
                    signed,
                };
            }
            "--result-length-register" => {
                let length_register = parse_result_register(&args.next().ok_or_else(usage)?)?;
                let (ptr_register, elem_width, signed) = match parsed.result_spec.clone() {
                    ResultSpec::Array {
                        ptr_register,
                        elem_width,
                        signed,
                        ..
                    } => (ptr_register, elem_width, signed),
                    _ => (rizz_v::reg::Register::A0, 4, true),
                };
                parsed.result_spec = ResultSpec::Array {
                    ptr_register,
                    length_register,
                    elem_width,
                    signed,
                };
            }
            "--result-elem-width" => {
                let raw = args.next().ok_or_else(usage)?;
                let elem_width = raw
                    .parse::<u8>()
                    .map_err(|_| format!("invalid --result-elem-width value: {raw}"))?;
                let (ptr_register, length_register, signed) = match parsed.result_spec.clone() {
                    ResultSpec::Array {
                        ptr_register,
                        length_register,
                        signed,
                        ..
                    } => (ptr_register, length_register, signed),
                    _ => (rizz_v::reg::Register::A0, rizz_v::reg::Register::A1, true),
                };
                parsed.result_spec = ResultSpec::Array {
                    ptr_register,
                    length_register,
                    elem_width,
                    signed,
                };
            }
            "--result-unsigned" => {
                let (ptr_register, length_register, elem_width) = match parsed.result_spec.clone() {
                    ResultSpec::Array {
                        ptr_register,
                        length_register,
                        elem_width,
                        ..
                    } => (ptr_register, length_register, elem_width),
                    _ => (rizz_v::reg::Register::A0, rizz_v::reg::Register::A1, 4),
                };
                parsed.result_spec = ResultSpec::Array {
                    ptr_register,
                    length_register,
                    elem_width,
                    signed: false,
                };
            }
            "--help" | "-h" => return Err(usage()),
            _ => return Err(format!("unknown argument: {arg}\n{}", usage())),
        }
    }

    Ok(Command::Cli(parsed))
}

fn usage() -> String {
    "usage:\n  rizz-v serve [--port N]\n  rizz-v <input.s> [--asm-out path] [--trace-out path] [--analysis-out path] [--result-out path] [--max-steps N] [--reg name=value]... [--result-kind scalar|array] [--result-register reg] [--result-ptr-register reg] [--result-length-register reg] [--result-elem-width 1|2|4|8] [--result-unsigned]".to_string()
}
