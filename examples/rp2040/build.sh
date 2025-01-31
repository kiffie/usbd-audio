#!/bin/bash

# This script builds the project and converts it to a format suitable for the Raspberry Pi Pico
#
# Prerequisites:
# 1. Install elf2uf2-rs tool:
#    cargo install elf2uf2-rs
#
# Why elf2uf2-rs?
# The Raspberry Pi Pico expects firmware in the UF2 format. This tool converts our compiled ELF
# binary into the UF2 format that the Pico's bootloader can flash.
#
# Deployment instructions:
# 1. Hold the BOOTSEL button on the Pico while connecting it to USB
# 2. The Pico will appear as a USB mass storage device
# 3. Copy the generated .uf2 file to the Pico drive
# 4. The Pico will automatically restart and run the new firmware

BIN=usb-audio-example

cargo build --release

elf2uf2-rs ./target/thumbv6m-none-eabi/release/usb-audio ./target/$BIN.uf2
