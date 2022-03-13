


#define _GNU_SOURCE

/* These libraries are necessary for the hook */
#include <dlfcn.h>
#include <stdlib.h>
#include <GL/gl.h>

/* "Injected" stuff */
#include <stdio.h>
#include <stdint.h>
#include <string.h>

static int framecnt = 0;

// our detour function
void HackFrame() {
        // do our stuff
	framecnt++;
	
	printf("frame %d... \n", framecnt);
}

// hook glClear
void glClear(GLbitfield mask) {
		static void (*lib_glClear)(GLbitfield mask) = NULL;
	void* handle;
	char* errorstr;

	if(!lib_glClear) {
		/* Load real libGL */
		handle = dlopen("/usr/lib/libGL.so", RTLD_LAZY);
		if(!handle) {
			fputs(dlerror(), stderr);
			exit(1);
		}
		/* Fetch pointer of real glClear() func */
		lib_glClear = dlsym(handle, "glClear");
		if( (errorstr = dlerror()) != NULL ) {
			fprintf(stderr, "dlsym fail: %s\n", errorstr);
			exit(1);
		}
	}

	/* Woot */
	HackFrame();

	/* Call real glClear() */
	lib_glClear(mask);
}

