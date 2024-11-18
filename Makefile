CURRENT_DIR := $(shell pwd)

image:
	docker build -t ll .

dist:
	docker run --rm -e SDKROOT=/app/MacOSX.sdk -v $(CURRENT_DIR)/build:/app/build ll bin/build

console:
	docker run -it --rm -e SDKROOT=/app/MacOSX.sdk -v $(CURRENT_DIR)/build:/app/build ll /bin/bash
