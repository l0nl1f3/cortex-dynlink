int GLOBAL_X = 20;
int GLOBAL_Y = 31;
int z_GLOBAL_Z = 10;

int inc(int x)  // { return x; };
{
  GLOBAL_X += x;
}

int test(int x) {
  inc(x);
  return GLOBAL_X + z_GLOBAL_Z == GLOBAL_Y;
}