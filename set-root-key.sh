#! /bin/sh

while [ $# -ne 0 ]; do
case $1 in
'--host')
    rhost="$2"
    shift; shift
    ;;
'--key')
    key="$2"
    shift;shift
    ;;
'--help|-h')
    echo "usage: $0 --host rhost --key ssh_key"
    exit 0
    ;;
*)
    echo "invalid parameter $*"
    exit 255
esac
done

if [ "${rhost}@" = '@' ]; then
    echo "no remote host"
    exit 255
fi

key=${key:-"$HOME/.ssh/id_rsa"}
if [ ! -r "$key" -o ! -r "$key.pub" ]; then
    echo "invalid key \"$key\""
    exit 255
fi

ssh_params='-o PasswordAuthentication=no -o LogLevel=error'
ssh_params="${ssh_params} -i $key"

ssh $ssh_params $rhost 'exit'
if [ $? -ne 0 ]; then
  echo "user \"$USER\" cannot ssh to \"$rhost\""
  exit 255
fi

ssh $ssh_params $rhost 'sudo -n echo'
if [ $? -ne 0 ]; then
  echo "user \"$USER\" cannot sudo on \"$rhost\""
  exit 255
fi

key_file=$(mktemp --suffix "_${USER}.tmp")
ssh $ssh_params "$rhost" 'sudo -n cat /root/.ssh/authorized_keys' | grep -q "$(cat $key.pub)"
if [ $? -ne 0 ]; then
  echo "\"root@$rhost\" add key \"$key.pub\""
  scp -q $ssh_params $key.pub "$rhost:$key_file"
  echo "cat $key_file >> /root/.ssh/authorized_keys; rm -f $key_file" | \
  xargs -I'{}' ssh $ssh_params "$rhost" "sudo -n sh -c \"{}\""
else
  echo "\"root@$rhost\" key exists \"$key.pub\""
fi
rm -f "$key_file"
