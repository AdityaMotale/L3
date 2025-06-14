#include <stdint.h>
#include <stdio.h>
#include <time.h>
#include <x86intrin.h>

extern uint32_t popcnt_v1(uint32_t);
extern uint32_t popcnt_v2(uint32_t);
extern uint32_t popcnt_hw(uint32_t);

static const uint32_t TEST_VAL = 0xF0F0F0F0u;
static const size_t N = 500000000;

// helper to avoid compiler optimizations
static uint32_t sink = 0;

uint64_t bench_time(uint32_t (*fn)(uint32_t)) {
  struct timespec t0, t1;
  clock_gettime(CLOCK_MONOTONIC_RAW, &t0);

  for (size_t i = 0; i < N; i++) {
    sink ^= fn(TEST_VAL ^ (uint32_t)i);
  }

  clock_gettime(CLOCK_MONOTONIC_RAW, &t1);
  uint64_t elapsed_ns =
      (t1.tv_sec - t0.tv_sec) * 1000000000ull + (t1.tv_nsec - t0.tv_nsec);

  return elapsed_ns;
}

int main(void) {
  uint64_t t, c;

  t = bench_time(popcnt_v1);
  printf("v1: time = %.2f ms\n", t / 1e6);

  t = bench_time(popcnt_v2);
  printf("v2: time = %.2f ms\n", t / 1e6);

  t = bench_time(popcnt_hw);
  printf("hw: time = %.2f ms\n", t / 1e6);

  if (sink == 0xBADBEEF) {
    printf("weird\n");
  }

  return 0;
}
