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

int main(void) {
  printf("POPCNT 24 -> %d\n", popcnt_v1(13));
  return 0;
}
