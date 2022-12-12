#!/bin/bash
./build.sh rpi3-raspbian
exec_name=`basename $1`
scp $1 pi@iaa34.local:/home/pi
ssh -X -t -o SendEnv="RUST_LOG RUST_BACKTRACE" pi@iaa34.local DISPLAY=:0 /home/pi/$exec_name
