<!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.0 Transitional//EN" "http://www.w3.org/TR/xhtml1/DTD/xhtml1-transitional.dtd">
<html xmlns="http://www.w3.org/1999/xhtml">
<head>
<meta http-equiv="X-UA-Compatible" content="IE=Edge"/>
<meta http-equiv="Content-Type" content="text/html; charset=utf-8" />
<meta HTTP-EQUIV="Pragma" CONTENT="no-cache"/>
<meta HTTP-EQUIV="Expires" CONTENT="-1"/>
<title>软件中心 - MagicTier</title>
<link rel="stylesheet" type="text/css" href="index_style.css"/>
<link rel="stylesheet" type="text/css" href="form_style.css"/>
<link rel="stylesheet" type="text/css" href="res/softcenter.css"/>
<script type="text/javascript" src="/js/jquery.js"></script>
<script type="text/javascript" src="/state.js"></script>
<script type="text/javascript" src="/popup.js"></script>
<script type="text/javascript" src="/general.js"></script>
<script type="text/javascript" src="/res/softcenter.js"></script>
<style type="text/css">
#magictier_log_mask{display:none;position:fixed;z-index:998;left:0;top:0;width:100%;height:100%;background:rgba(0,0,0,.55)}
#magictier_log_box{display:none;position:fixed;z-index:999;left:50%;top:50%;transform:translate(-50%,-50%);width:720px;max-width:92%;background:#2f3a3e;border-radius:4px;padding:12px;box-shadow:0 0 18px #000}
#magictier_log_text{width:98%;height:360px;background:#000;color:#fff;border:1px solid #666;font-family:monospace;font-size:12px;resize:none}
</style>
<script type="text/javascript">
var db_magictier = {};
var statusTimer = null;
function E(id){return document.getElementById(id);}
function menu_hook(){tabtitle[tabtitle.length-1]=new Array("","MagicTier");tablink[tablink.length-1]=new Array("","Module_magictier.asp");}
function api(method, params, fields, done){
    $.ajax({url:"/_api/",cache:false,type:"POST",dataType:"json",data:JSON.stringify({id:parseInt(Math.random()*100000000),method:method,params:params||[],fields:fields||{}}),complete:function(xhr){if(done)done(xhr);}});
}
function init(){
    show_menu(menu_hook);
    $.ajax({type:"GET",url:"/_api/magictier",dataType:"json",async:false,success:function(data){
        db_magictier=(data.result&&data.result[0])?data.result[0]:{};
        E("magictier_enable").checked=db_magictier.magictier_enable=="1";
        ["network_name","network_secret","ipv4","peers","listeners","proxy_networks"].forEach(function(k){if(db_magictier["magictier_"+k]!==undefined)E("magictier_"+k).value=db_magictier["magictier_"+k];});
        E("magictier_version").innerHTML=db_magictier.magictier_version||"-";
    }});
    refresh_status();
    statusTimer=setInterval(refresh_status,5000);
}
function save(){
    db_magictier.magictier_enable=E("magictier_enable").checked?"1":"0";
    ["network_name","network_secret","ipv4","peers","listeners","proxy_networks"].forEach(function(k){db_magictier["magictier_"+k]=E("magictier_"+k).value;});
    showLoading(3);
    api("magictier_config.sh",[1],db_magictier,function(){setTimeout(function(){location.reload();},2200);});
}
function service_action(action){
    var code=action=="start"?2:action=="stop"?3:4;
    api("magictier_config.sh",[code],{},function(){setTimeout(refresh_status,1200);show_log();});
}
function refresh_status(){
    api("magictier_config.sh",[6],{},function(xhr){
        var t=xhr.responseText||"";
        var m=t.match(/\"state\"\s*:\s*\"(running|stopped)\"[^}]*\"pid\"\s*:\s*(\d+)[^}]*\"rss_kb\"\s*:\s*(\d+)/);
        if(m){
            E("run_state").innerHTML=m[1]=="running"?"运行中":"已停止";
            E("run_pid").innerHTML=m[2];
            E("run_rss").innerHTML=(parseInt(m[3],10)/1024).toFixed(1)+" MB";
        }
    });
}
function show_log(){
    E("magictier_log_mask").style.display="block";E("magictier_log_box").style.display="block";load_log();
}
function hide_log(){E("magictier_log_mask").style.display="none";E("magictier_log_box").style.display="none";}
function load_log(){
    $.ajax({url:"/_temp/magictier_log.txt?_="+new Date().getTime(),type:"GET",cache:false,dataType:"text",success:function(t){E("magictier_log_text").value=t||"";E("magictier_log_text").scrollTop=E("magictier_log_text").scrollHeight;},error:function(){E("magictier_log_text").value="暂无日志";}});
}
function clear_log(){api("magictier_config.sh",[5],{},function(){setTimeout(load_log,200);});}
function reload_Soft_Center(){location.href="/Module_Softcenter.asp";}
</script>
</head>
<body onload="init();">
<div id="TopBanner"></div><div id="Loading" class="popup_bg"></div>
<div id="magictier_log_mask" onclick="hide_log();"></div>
<div id="magictier_log_box"><div style="color:#fff;font-size:16px;margin-bottom:8px;">MagicTier 运行日志</div><textarea id="magictier_log_text" readonly="readonly"></textarea><div style="text-align:center;margin-top:10px;"><input class="button_gen" type="button" onclick="load_log();" value="刷新" />&nbsp;<input class="button_gen" type="button" onclick="clear_log();" value="清空日志" />&nbsp;<input class="button_gen" type="button" onclick="hide_log();" value="关闭" /></div></div>
<table class="content" align="center" cellpadding="0" cellspacing="0"><tr><td width="17">&nbsp;</td><td valign="top" width="202"><div id="mainMenu"></div><div id="subMenu"></div></td><td valign="top"><div id="tabMenu" class="submenuBlock"></div>
<table width="98%" border="0" align="left" cellpadding="0" cellspacing="0"><tr><td align="left" valign="top"><table width="760px" border="0" cellpadding="5" cellspacing="0" class="FormTitle"><tr><td bgcolor="#4D595D" colspan="3" valign="top">
<div>&nbsp;</div><div style="float:left;" class="formfonttitle">MagicTier</div><div style="float:right;width:15px;height:25px;margin-top:10px"><img onclick="reload_Soft_Center();" align="right" style="cursor:pointer;position:absolute;margin-left:-30px;margin-top:-25px;" title="返回软件中心" src="/images/backprev.png" /></div><div style="margin:30px 0 10px 5px;" class="splitLine"></div>
<div class="formfontdesc">MagicTier ARM64 组网插件。当前版本：<span id="magictier_version">-</span></div>
<table style="margin-top:10px;" width="100%" border="1" align="center" cellpadding="4" cellspacing="0" bordercolor="#6b8fa3" class="FormTable"><thead><tr><td colspan="2">运行状态</td></tr></thead>
<tr><th>状态</th><td><span id="run_state">检测中</span>　PID: <span id="run_pid">-</span>　RSS: <span id="run_rss">-</span></td></tr>
<tr><th>操作</th><td><input class="button_gen" type="button" onclick="service_action('start');" value="启动" />&nbsp;<input class="button_gen" type="button" onclick="service_action('stop');" value="停止" />&nbsp;<input class="button_gen" type="button" onclick="service_action('restart');" value="重启" />&nbsp;<input class="button_gen" type="button" onclick="show_log();" value="查看日志" /></td></tr></table>
<table style="margin-top:10px;" width="100%" border="1" align="center" cellpadding="4" cellspacing="0" bordercolor="#6b8fa3" class="FormTable"><thead><tr><td colspan="2">运行设置</td></tr></thead>
<tr><th>启用 MagicTier</th><td><input id="magictier_enable" type="checkbox" /></td></tr>
<tr><th>网络名称</th><td><input id="magictier_network_name" class="input_ss_table" maxlength="128" /></td></tr>
<tr><th>网络密钥</th><td><input id="magictier_network_secret" type="password" class="input_ss_table" maxlength="256" autocomplete="new-password" /></td></tr>
<tr><th>虚拟 IPv4</th><td><input id="magictier_ipv4" class="input_ss_table" maxlength="64" placeholder="10.144.144.1/24" /></td></tr>
<tr><th>Peer 节点</th><td><input id="magictier_peers" class="input_ss_table" style="width:420px" maxlength="1024" placeholder="tcp://1.2.3.4:11010" /></td></tr>
<tr><th>监听地址</th><td><input id="magictier_listeners" class="input_ss_table" style="width:420px" maxlength="1024" placeholder="tcp://0.0.0.0:11010,udp://0.0.0.0:11010" /></td></tr>
<tr><th>发布子网</th><td><input id="magictier_proxy_networks" class="input_ss_table" style="width:420px" maxlength="512" placeholder="192.168.50.0/24" /></td></tr></table>
<div style="margin-top:15px;text-align:center;"><input class="button_gen" type="button" onclick="save();" value="保存并应用" /></div>
<div style="margin:15px 0 5px 0;" class="formfontdesc">7×24保护：控制台仅记录 WARN 以上；RAM 日志上限约 512KB；默认 RSS 超过 256MB 自动停服并关闭自动启动；进程以较低 CPU 优先级运行。</div>
</td></tr></table></td></tr></table></td></tr></table><div id="footer"></div>
</body></html>
