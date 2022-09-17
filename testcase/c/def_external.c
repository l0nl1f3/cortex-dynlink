int GLOBAL_X = 20;

int inc(int x)  // { return x; };
{
  GLOBAL_X += x;
  return GLOBAL_X;
}
