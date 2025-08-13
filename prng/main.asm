bits 64
default rel

section .rodata
        GOLDEN_RATIO: dq 0x9E3779B97F4A7C15
        MULT_ONE: dq 0xBF58476D1CE4E5B9
        MULT_TWO: dq 0x94D049BB133111EB

section .bss
        time_val resq 2                     ; 16-bytes for `tval` struct
        out_buf resb 22                     ; output buffer to print the ASCII u64 number
        seeds resq 8                        ; eight 64-bit slot for eight sub-seeds
        rns resq 4                          ; four 64-bit slot for generated random numbers

section .text
        global _start

_start:
        ; obtain current epoch time
        ; using `clock_gettime` syscall
        mov rax, 0xE4
        xor rdi, rdi
        lea rsi, [time_val]
        syscall

        ; generate first 4 seeds
        mov rdi, [time_val + 8]             ; use nanoseconds as initial seed
        lea rsi, [seeds]
        call function_split_mix_64

        ; generate next 4 seeds,
        ; here we use 3rd seed as the base seed
        mov rdi, [seeds + 24]
        lea rsi, [seeds + 32]
        call function_split_mix_64 

        lea rsi, [seeds]
        lea rdi, [rns]
        call function_xoroshiro_128_plus

        lea r12, [rns]
        mov r13, 0x00                       ; loop counter (0)

.print_loop:
        ; format seed[i]
        mov rdi, [r12 + r13 * 8]
        lea rsi, [out_buf]
        call function_itoa

        ; print seed[i]
        mov rdx, rax
        mov rax, 0x01
        mov rdi, 0x01
        syscall

        inc r13

        cmp r13, 0x04
        jl .print_loop
         
.exit:        
        ; exit(0)
        mov rax, 0x3C
        xor rdi, rdi
        syscall


; Generate four random 64-bit numbers using AVX2 and eight 64-bit seeds
;
; Args:
;   rdi - pointer to 32-bytes output buffer (4 * u64)
;   rsi - pointer to 64-bytes seeds buffer (8 * u64 seeds)
;
; Returns:
;   rax - `-1` on error, 0 otherwise 
;
; Clobbers:
;   ymm0-ymm7, rax, rcx, rdx
function_xoroshiro_128_plus:
        ; load s0 & s1 (first four & last four)
        vmovdqu ymm0, [rsi]                ; s0 lane
        vmovdqu ymm1, [rsi + 32]           ; s1 lane 

        ; result = s0 + s1
        vpaddq ymm2, ymm0, ymm1
        vmovdqu [rdi], ymm2                ; move 32 bytes (4 u64 nums) into out buf

        vpxor ymm1, ymm1, ymm0             ; s1 ^= s0 (in-place)

        ; temp_1 = rol(s0, 55), which is `(s0 << 55) | (s0 >> 9)`
        vpsllq ymm3, ymm0, 55              ; ymm3 = s0 << 55
        vpsrlq ymm4, ymm0, 9               ; ymm4 = s0 >> 9
        vpor ymm5, ymm3, ymm2              ; ymm5 = rol(s0, 55)

        ; temp_2 = s1 << 14
        vpsllq ymm6, ymm1, 14

        ; new s0 lane = `temp_1 ^ s1 ^ (s1 << 14)`
        vpxor ymm5, ymm5, ymm1              ; ymm5 ^= s1
        vpxor ymm5, ymm5, ymm6              ; ymm6 ^= (s1 << 14)

        ; new s1 lane = `rol(s1, 36)`
        vpsllq ymm6, ymm1, 36
        vpsrlq ymm7, ymm1, 28               ; 64 - 36 = 28
        vpor ymm1, ymm6, ymm7               ; ymm1 = rol(s1, 36)

        ; store back new state
        vmovdqu [rsi], ymm5
        vmovdqu [rsi + 32], ymm1

        ; avoid SSE transition penalty
        vzeroupper
        
        xor rax, rax
        ret


; Generate four independent 64-bit "sub-seeds" based on input seed
;
; Args:
;   rdi - initial 64-bit seed
;   rsi - pointer to 4 * 8 byte buffer
;
; Returns:
;   rsi - (preserved) pointer to buffer w/ 4 * 8 byte values written
;
; Clobbers:
;   rax, rcx, rdx, r8
function_split_mix_64:
        mov rax, rdi                        ; z = initial_seed
        mov rcx, 0x00                       ; loop counter (0)

.seed_loop:
        lea r8, [GOLDEN_RATIO]
        add rax, [r8]                       ; z += golden_ratio 

        mov rdx, rax
        shr rdx, 0x1E                       ; shift right by 30 
        xor rax, rdx

        lea r8, [MULT_ONE]
        imul rax, [r8]

        mov rdx, rax
        shr rdx, 0x1B                       ; shift right by 27
        xor rax, rdx
        
        lea r8, [MULT_TWO]
        imul rax, [r8]

        mov rdx, rax
        shr rdx, 0x1F                       ; shift right by 31 
        xor rax, rdx

        mov [rsi + rcx * 8], rax            ; store out[i] 
        inc rcx

        cmp rcx, 0x04
        jl .seed_loop

        ; return
        ret
        
; Convert a 64-bit unsigned integer to ASCII string
;
; Args:
;   rdi - 64-bit unsigned integer 
;   rsi - Pointer to the output buffer (must be >= 21 bytes)
;
; Returns:
;   rax - Length of the output buf
;   rsi - (preserved) pointer to the output buffer
;
; Clobbers:
;   rax, rbx, rcx, rdx, r8–r11
function_itoa:
        mov r8, rsi                         ; buf start
        lea r9, [rsi + 20]                  ; buf end
        mov rbx, rdi                        ; current working value

        ; special "zero-case"
        test rbx, rbx
        jnz .convert_loop

        ; if u64 == 0 -> "0\n"
        mov byte [r9-2], '0'
        mov byte [r9-1], 0x00
        mov rax, 0x01
        mov rsi, r8
        ret

.convert_loop:
        mov r10, r9

.loop:
        mov r11, rbx

        ; compute quotient,
        ; q = [rbx / 0x0A]
        mov rax, rbx
        mov rcx, 0xCCCCCCCCCCCCCCCD
        mul rcx
        shr rdx, 0x03                       ; rdx (quotient) = high‑part >> 3
        mov rbx, rdx                        ; new_val = quotient

        ; compute remainder,
        ; rem = [original − quotient*10]
        mov rax, r11                        ; original
        mov rcx, rdx                        ; quotient
        imul rcx, 0x0A                      ; rcx = q * 10
        sub rax, rcx                        ; rax = remainder (0..9)

        ; compute & store the ascii digit
        add al, '0'                         ; ascii digit
        dec r10
        mov [r10], al

        ; loop til quotient > 0
        test rbx, rbx
        jnz .loop

        lea rax, [r9 - 1]                   ; pointer just past last digit
        sub rax, r10                        ; rax = len

        ; copy them forward with MOVSB
        mov rsi, r10                        ; src
        mov rdi, r8                         ; dst
        mov rcx, rax                        ; count
        rep movsb

        ; write trailing NULL
        mov byte [r8 + rax], 0x0A
        inc rax

        mov rsi, r8
        ret
