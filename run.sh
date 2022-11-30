#!/bin/bash
exec_name=`basename $1`
scp $1 pi@iaa34.local:/home/pi
ssh pi@iaa34.local /home/pi/$exec_name
