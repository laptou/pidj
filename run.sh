#!/bin/bash
exec_name=`basename $1`
scp $1 pi@iaa34.local:/home/pi
ssh -t pi@iaa34.local "RUST_LOG=pidj=trace /home/pi/$exec_name"
