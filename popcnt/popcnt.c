#include <stdint.h>
#include <stdio.h>

uint32_t popcnt_v1(uint32_t x) {
  uint32_t total = 0;

  while (x) {
    total++;
    x &= (x - 1);
  }

  return total;
}

uint32_t popcnt_v2(uint32_t x) {
  // pair up bits w/ 2-bit sum
  uint32_t even = x & 0x55555555;
  uint32_t odd = (x >> 1) & 0x55555555;
  x = even + odd;

  // group into 4-bits sum
  even = x & 0x33333333;
  odd = (x >> 2) & 0x33333333;
  x = even + odd;

  // group into 8-bits sum
  even = x & 0x0F0F0F0F;
  odd = (x >> 4) & 0x0F0F0F0F;
  x = even + odd;

  // group into 16-bits sum
  even = x & 0x00FF00FF;
  odd = (x >> 8) & 0x00FF00FF;
  x = even + odd;

  // group into 32-bits sum
  even = x & 0x0000FFFF;
  odd = (x >> 16) & 0x0000FFFF;
  x = even + odd;

  return x;
}

int main(void) {
  printf("POPCNT (v1) 24 -> %d\n", popcnt_v1(13));
  printf("POPCNT (v1) 24 -> %d\n", popcnt_v2(13));
  return 0;
}
