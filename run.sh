#!/bin/bash

#we build our binary
cargo b --release
# if compilation failes we stop (and not run anything already compiled previously)
ext=$?
if [ $ext -ne 0 ]
then
    exit $ext
fi

# we allow our binary to do network things (neccessary for tun tap)
sudo setcap cap_net_admin=eip ./target/release/rs_tcp
# we run the compiled file and keep its pid (process id) arround
./target/release/rs_tcp &
pid=$!
# we set up the tun connection
sudo ip add add 192.168.0.1/24 dev tun0
# we send over a packet
sudo ip link set up dev tun0


# in case we receive ^C we must manually clean up our process (basically we pass on ^C)
trap "kill $pid" INT TERM
# we wait for process to exit 
wait $pid
