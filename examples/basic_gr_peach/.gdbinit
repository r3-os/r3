target remote :3333
mon reset halt

# Enable the on-chip RAM
mon mwb 0xFCFE0400 0xff
mon mwb 0xFCFE0404 0xff
mon mwb 0xFCFE0408 0xff

load
