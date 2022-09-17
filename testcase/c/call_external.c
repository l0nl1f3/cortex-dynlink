int GLOBAL_Y = 31;
int GLOBAL_Z = 10;

int inc(int x);
int test(int x) { return inc(x) + GLOBAL_Z == GLOBAL_Y; }