use std::collections::{BTreeSet, HashMap, HashSet};

use serde::Serialize;

use crate::assembler::Assembler;
use crate::instruction::{Instruction, JumpTarget};

const WORD_SIZE: u64 = 4;

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct BasicBlock {
    pub id: usize,
    pub start_pc: u64,
    pub end_pc: u64,
    pub instruction_pcs: Vec<u64>,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct Edge {
    pub from: usize,
    pub to: usize,
    pub kind: EdgeKind,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub enum EdgeKind {
    Fallthrough,
    BranchTaken,
    BranchNotTaken,
    Jump,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct LoopSummary {
    pub header: usize,
    pub back_edge_from: usize,
    pub blocks: Vec<usize>,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct ControlFlowAnalysis {
    pub entry_block: usize,
    pub blocks: Vec<BasicBlock>,
    pub edges: Vec<Edge>,
    pub loops: Vec<LoopSummary>,
}

pub fn analyze(assembler: &Assembler) -> ControlFlowAnalysis {
    let instructions = assembler.instructions();
    let entry_pc = assembler.entry_pc();

    if instructions.is_empty() {
        return ControlFlowAnalysis {
            entry_block: 0,
            blocks: Vec::new(),
            edges: Vec::new(),
            loops: Vec::new(),
        };
    }

    let mut leaders = BTreeSet::from([entry_pc]);
    let last_pc = ((instructions.len() - 1) as u64) * WORD_SIZE;

    for (index, instruction) in instructions.iter().enumerate() {
        let pc = (index as u64) * WORD_SIZE;
        let fallthrough = pc + WORD_SIZE;
        match instruction {
            Instruction::B(branch) => {
                if let Some(target) = branch.target.address() {
                    leaders.insert(target);
                }
                if fallthrough <= last_pc {
                    leaders.insert(fallthrough);
                }
            }
            Instruction::J(jump) => match &jump.target {
                JumpTarget::Direct(target) => {
                    if let Some(address) = target.address() {
                        leaders.insert(address);
                    }
                }
                JumpTarget::Indirect(_) => {}
            },
            _ => {}
        }
    }

    let leader_vec = leaders.into_iter().collect::<Vec<_>>();
    let mut blocks = Vec::with_capacity(leader_vec.len());
    for (idx, start_pc) in leader_vec.iter().enumerate() {
        let end_pc = leader_vec
            .get(idx + 1)
            .map(|next| next.saturating_sub(WORD_SIZE))
            .unwrap_or(last_pc);
        let instruction_pcs = (*start_pc..=end_pc).step_by(WORD_SIZE as usize).collect();
        blocks.push(BasicBlock {
            id: idx,
            start_pc: *start_pc,
            end_pc,
            instruction_pcs,
        });
    }

    let mut pc_to_block = HashMap::new();
    for block in &blocks {
        for pc in &block.instruction_pcs {
            pc_to_block.insert(*pc, block.id);
        }
    }

    let mut edges = Vec::new();
    for block in &blocks {
        let last_pc = block.end_pc;
        let Some(instruction) = assembler.instruction_at_pc(last_pc) else {
            continue;
        };
        match instruction {
            Instruction::B(branch) => {
                if let Some(target_pc) = branch.target.address() {
                    if let Some(target_block) = pc_to_block.get(&target_pc) {
                        edges.push(Edge {
                            from: block.id,
                            to: *target_block,
                            kind: EdgeKind::BranchTaken,
                        });
                    }
                }
                let fallthrough_pc = last_pc + WORD_SIZE;
                if let Some(target_block) = pc_to_block.get(&fallthrough_pc) {
                    edges.push(Edge {
                        from: block.id,
                        to: *target_block,
                        kind: EdgeKind::BranchNotTaken,
                    });
                }
            }
            Instruction::J(jump) => match &jump.target {
                JumpTarget::Direct(target) => {
                    if let Some(target_pc) = target.address() {
                        if let Some(target_block) = pc_to_block.get(&target_pc) {
                            edges.push(Edge {
                                from: block.id,
                                to: *target_block,
                                kind: EdgeKind::Jump,
                            });
                        }
                    }
                }
                JumpTarget::Indirect(_) => {}
            },
            _ => {
                let fallthrough_pc = last_pc + WORD_SIZE;
                if let Some(target_block) = pc_to_block.get(&fallthrough_pc) {
                    edges.push(Edge {
                        from: block.id,
                        to: *target_block,
                        kind: EdgeKind::Fallthrough,
                    });
                }
            }
        }
    }

    let loops = find_loops(&blocks, &edges, assembler.entry_pc());
    let entry_block = pc_to_block.get(&entry_pc).copied().unwrap_or(0);

    ControlFlowAnalysis {
        entry_block,
        blocks,
        edges,
        loops,
    }
}

fn find_loops(blocks: &[BasicBlock], edges: &[Edge], entry_pc: u64) -> Vec<LoopSummary> {
    let mut adjacency: HashMap<usize, Vec<usize>> = HashMap::new();
    for edge in edges {
        adjacency.entry(edge.from).or_default().push(edge.to);
    }

    let mut start_block = 0;
    for block in blocks {
        if block.start_pc == entry_pc {
            start_block = block.id;
            break;
        }
    }

    let mut visited = HashSet::new();
    let mut stack = Vec::new();
    let mut back_edges = Vec::new();
    dfs(start_block, &adjacency, &mut visited, &mut stack, &mut back_edges);

    let reverse = reverse_edges(edges);
    let mut loops = Vec::new();
    for (from, header) in back_edges {
        let mut members = HashSet::from([header]);
        let mut work = vec![from];
        while let Some(block) = work.pop() {
            if members.insert(block) {
                if let Some(preds) = reverse.get(&block) {
                    for pred in preds {
                        if *pred != header {
                            work.push(*pred);
                        }
                    }
                }
            }
        }
        let mut blocks = members.into_iter().collect::<Vec<_>>();
        blocks.sort_unstable();
        loops.push(LoopSummary {
            header,
            back_edge_from: from,
            blocks,
        });
    }
    loops.sort_by_key(|summary| (summary.header, summary.back_edge_from));
    loops
}

fn dfs(
    node: usize,
    adjacency: &HashMap<usize, Vec<usize>>,
    visited: &mut HashSet<usize>,
    stack: &mut Vec<usize>,
    back_edges: &mut Vec<(usize, usize)>,
) {
    visited.insert(node);
    stack.push(node);

    if let Some(neighbors) = adjacency.get(&node) {
        for neighbor in neighbors {
            if stack.contains(neighbor) {
                back_edges.push((node, *neighbor));
                continue;
            }
            if !visited.contains(neighbor) {
                dfs(*neighbor, adjacency, visited, stack, back_edges);
            }
        }
    }

    stack.pop();
}

fn reverse_edges(edges: &[Edge]) -> HashMap<usize, Vec<usize>> {
    let mut reverse = HashMap::new();
    for edge in edges {
        reverse.entry(edge.to).or_insert_with(Vec::new).push(edge.from);
    }
    reverse
}
