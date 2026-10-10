.PHONY: all clean

all:
	./tools/build-release.sh

clean:
	cargo clean
	rm -vrf ./dist
