// from mango hud source code

#include "elfhacks.h"
#include <stdio.h>
#include <stdlib.h>

void *(*__dlsym)(void *, const char *) = NULL;

void get_dlsym()
{
    eh_obj_t libdl;
    int ret;

    const char *libs[] = {
#if defined(__GLIBC__)
        "*libdl.so*",
#endif
        "*libc.so*",
        "*libc.*.so*",
    };

    for (size_t i = 0; i < sizeof(libs) / sizeof(*libs); i++) {
        ret = eh_find_obj(&libdl, libs[i]);
        if (ret)
            continue;

        eh_find_sym(&libdl, "dlsym", (void **)&__dlsym);
        eh_destroy_obj(&libdl);

        if (__dlsym)
            break;
        __dlsym = NULL;
    }

    if (!__dlsym) {
        fprintf(stderr, "~~ Error : dlsym was not found ~~\n");
        exit(ret ? ret : 1);
    }
}

void *real_dlsym(void *handle, const char *symbol)
{
    if (__dlsym == NULL)
        get_dlsym();

    return (__dlsym)(handle, symbol);
}
