#!/bin/bash
exec_name=`basename $1`
scp $1 pi@iaa34.local:/home/pi
ssh -t -o SendEnv=RUST_LOG pi@iaa34.local /home/pi/$exec_name
