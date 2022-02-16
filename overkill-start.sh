#!/usr/bin/bash

hservice=peer-hidden-service-address
gladedefault=/usr/local/share/overkill/starter.glade
cssdefault=/usr/local/share/overkill/starter.css
namedpipe=/tmp/overkill-pipe

trap "rm $namedpipe" EXIT
mkfifo $namedpipe
if [[ ( -e $gladedefault ) && ( -e $cssdefault ) ]]
then 
    env gladefile=$gladedefault cssfile=$cssdefault overkill-gui < $namedpipe | overkill-chat -h $hservice > $namedpipe
else 
    ./overkill-gui < $namedpipe | ./overkill-chat -h $hservice > $namedpipe
fi