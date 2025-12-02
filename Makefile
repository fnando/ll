CURRENT_DIR := $(shell pwd)

image:
	docker build -t ll .

dist:
	docker run --rm -e SDKROOT=/app/MacOSX.sdk -v $(CURRENT_DIR):/app ll bin/build

console:
	docker run -it --rm -e SDKROOT=/app/MacOSX.sdk -v $(CURRENT_DIR):/app ll /bin/bash
