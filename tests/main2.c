#include <dlfcn.h>
#include <stdio.h>

void glXSwapBuffers(void *dpy, void *drawable) {
  static void (*glx_swap_buffers)(void *dpy, void *drawable) = 0;
  printf("test");
  void *handle = dlopen("libGL.so.1", RTLD_LAZY);
  glx_swap_buffers = dlsym(handle, "glXSwapBuffers");
  glx_swap_buffers(dpy, drawable);
}
