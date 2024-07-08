CC=gcc
CFLAGS=-Wextra -fPIC

real_dlsym: elfhacks.c main.c
	$(CC) -c elfhacks.c -o elfhacks.o $(CFLAGS)
	$(CC) -c main.c -o main.o $(CFLAGS)
	ar ruv librdlsym.a elfhacks.o main.o
	ranlib librdlsym.a

