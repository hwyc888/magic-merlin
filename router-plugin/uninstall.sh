#!/bin/sh

source /koolshare/scripts/base.sh

if [ -x /koolshare/scripts/magictier_config.sh ]; then
    sh /koolshare/scripts/magictier_config.sh stop >/dev/null 2>&1
fi

rm -f /koolshare/init.d/S97magictier.sh /koolshare/init.d/N97magictier.sh
rm -f /koolshare/bin/magictier-core /koolshare/bin/magictier-cli
rm -f /koolshare/scripts/magictier_config.sh /koolshare/scripts/magictier_health.sh /koolshare/scripts/uninstall_magictier.sh
rm -f /koolshare/webs/Module_magictier.asp /koolshare/res/icon-magictier.png
if [ -f /koolshare/magic/.magictier-owned ]; then
    rm -rf /koolshare/magic
fi
rm -f /tmp/upload/magictier_log.txt /tmp/upload/magictier_internal.log /tmp/upload/magictier_health.txt

dbus remove magictier >/dev/null 2>&1
dbus remove softcenter_module_magictier >/dev/null 2>&1

exit 0
