.global max3_s

# a0 - int a
# a1 - int b
# a2 - int c

max3_s:
    sd ra, (sp)
    jal max2
    ld ra, (sp)
    mv a1, a2
    sd ra, (sp)
    jal max2
    ld ra, (sp)
    j done


# a0 - int a
# a1 - int b
max2:
    bgt a0, a1, done
    mv a0, a1
    j done

done:
    ret
