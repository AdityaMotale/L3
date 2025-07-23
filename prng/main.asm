bits 64
default rel

section .bss
        time_val resq 2                ; 16-bytes for `tval` struct
        out_buf resb 22                ; output buffer to print the ASCII u64 number

section .text
        global _start

_start:
        ; obtain current epoch time
        ; using `clock_gettime` syscall
        mov rax, 0xE4
        xor rdi, rdi
        lea rsi, [time_val]
        syscall

        mov rdi, [time_val + 8]           ; use nanoseconds
        lea rsi, [out_buf]
        call function_itoa

        ; print to stdout
        ;
        ; NOTE: Assumes `rsi` holds pointer to [out_buf]
        mov rdx, rax                      ; sizeof(out_buf)
        mov rax, 0x01
        mov rdi, 0x01
        syscall

        ; exit(0)
        mov rax, 0x3C
        xor rdi, rdi
        syscall
        
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
        mov r8, rsi                       ; buf start
        lea r9, [rsi + 20]                ; buf end
        mov rbx, rdi                      ; current working value

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

        ; compute quotient = [rbx / 0x0A]
        mov rax, rbx
        mov rcx, 0xCCCCCCCCCCCCCCCD
        mul rcx
        shr rdx, 0x03                     ; rdx (quotient) = high‑part >> 3
        mov rbx, rdx                      ; new_val = quotient

        ; compute remainder = [original − quotient*10]
        mov rax, r11                      ; original
        mov rcx, rdx                      ; quotient
        imul rcx, 0x0A                    ; rcx = q * 10
        sub rax, rcx                      ; rax = remainder (0..9)

        ; compute & store the ascii digit
        add al, '0'                       ; ascii digit
        dec r10
        mov [r10], al

        ; loop til quotient > 0
        test rbx, rbx
        jnz .loop

        lea rax, [r9 - 1]                 ; pointer just past last digit
        sub rax, r10                      ; rax = len

        ; copy them forward with MOVSB
        mov rsi, r10                      ; src
        mov rdi, r8                       ; dst
        mov rcx, rax                      ; count
        rep movsb

        ; write trailing NULL
        mov byte [r8 + rax], 0x0A
        inc rax

        mov rsi, r8

        ret

