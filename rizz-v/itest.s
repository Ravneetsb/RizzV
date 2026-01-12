# Add four numbers
.global sadd4

# TEST
sadd4:
    add a0, a0, a1 # a0 <- a0 + a1
    add a0, a0, a2 # a0 <- a0 + a1 + a2
    add a0, a0, a3 # a0 <- a0 + a1 + a2 + a3
    ret
