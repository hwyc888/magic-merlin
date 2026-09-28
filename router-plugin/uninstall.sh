#!/bin/sh

source /koolshare/scripts/base.sh

if [ -x /koolshare/scripts/magic_config.sh ]; then
    sh /koolshare/scripts/magic_config.sh stop >/dev/null 2>&1
fi

rm -f /koolshare/init.d/S97magic.sh /koolshare/init.d/N97magic.sh
rm -f /koolshare/bin/magic-core /koolshare/bin/magic-cli
rm -f /koolshare/scripts/magic_config.sh /koolshare/scripts/magic_health.sh /koolshare/scripts/uninstall_magic.sh
rm -f /koolshare/webs/Module_magic.asp /koolshare/res/icon-magic.png
if [ -f /koolshare/magic/.magic-owned ]; then
    rm -rf /koolshare/magic
fi
rm -f /tmp/upload/magic_log.txt /tmp/upload/magic_internal.log /tmp/upload/magic_health.txt

dbus remove magic >/dev/null 2>&1
dbus remove softcenter_module_magic >/dev/null 2>&1

exit 0
