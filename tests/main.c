#include <dlfcn.h>
#include <stdio.h>

int main(int argc, char **argv) {
  int (*fn)(int);

  void *hd = dlopen("libzip.so", RTLD_NOW);
  fn = dlsym(hd, "zip_open");
  printf("success");

  return 0;
}
