#!/bin/bash

BIN=usb-audio-example

cargo objcopy --release -- -O ihex $BIN.hex
