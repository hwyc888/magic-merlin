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
#magictier_log_text{width:98%;height:360px;background:#000;color:#fff;border:1px solid #666;font-family:monospace;font-size:14px;line-height:1.6;resize:none}
#magictier_import_mask{display:none;position:fixed;z-index:998;left:0;top:0;width:100%;height:100%;background:rgba(0,0,0,.55)}
#magictier_import_box{display:none;position:fixed;z-index:999;left:50%;top:50%;transform:translate(-50%,-50%);width:720px;max-width:92%;background:#2f3a3e;border-radius:4px;padding:12px;box-shadow:0 0 18px #000}
#magictier_import_text{width:98%;height:360px;background:#111;color:#fff;border:1px solid #666;font-family:monospace;font-size:13px;resize:vertical}
#magictier_export_mask{display:none;position:fixed;z-index:998;left:0;top:0;width:100%;height:100%;background:rgba(0,0,0,.55)}
#magictier_export_box{display:none;position:fixed;z-index:999;left:50%;top:50%;transform:translate(-50%,-50%);width:720px;max-width:92%;background:#2f3a3e;border-radius:4px;padding:12px;box-shadow:0 0 18px #000}
#magictier_export_text{width:98%;height:360px;background:#111;color:#fff;border:1px solid #666;font-family:monospace;font-size:13px;line-height:1.5;resize:vertical}
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
        ["hostname","instance_name","network_name","network_secret","ipv4","peers","listeners","proxy_networks"].forEach(function(k){if(db_magictier["magictier_"+k]!==undefined)E("magictier_"+k).value=db_magictier["magictier_"+k];});
        E("magictier_version").innerHTML=db_magictier.magictier_version||"-";
    }});
    refresh_status();
    statusTimer=setInterval(refresh_status,5000);
}
function save(){
    db_magictier.magictier_enable=E("magictier_enable").checked?"1":"0";
    ["hostname","instance_name","network_name","network_secret","ipv4","peers","listeners","proxy_networks"].forEach(function(k){db_magictier["magictier_"+k]=E("magictier_"+k).value;});
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
function show_import(){E("magictier_import_mask").style.display="block";E("magictier_import_box").style.display="block";E("magictier_import_text").focus();}
function hide_import(){E("magictier_import_mask").style.display="none";E("magictier_import_box").style.display="none";}
function config_escape(v){return String(v||"").replace(/\\/g,"\\\\").replace(/\"/g,'\\\"').replace(/\r/g,"\\r").replace(/\n/g,"\\n").replace(/\t/g,"\\t");}
function split_config_list(v){
    var ret=[];
    String(v||"").split(",").forEach(function(x){x=x.replace(/^\s+|\s+$/g,"");if(x)ret.push(x);});
    return ret;
}
function build_config_text(){
    var hostname=E("magictier_hostname").value;
    var instanceName=E("magictier_instance_name").value;
    var networkName=E("magictier_network_name").value;
    var networkSecret=E("magictier_network_secret").value;
    var ipv4=E("magictier_ipv4").value;
    var listeners=split_config_list(E("magictier_listeners").value);
    var peers=split_config_list(E("magictier_peers").value);
    var proxyNetworks=split_config_list(E("magictier_proxy_networks").value);
    var lines=[];
    if(hostname)lines.push('hostname = "'+config_escape(hostname)+'"');
    if(instanceName)lines.push('instance_name = "'+config_escape(instanceName)+'"');
    if(ipv4)lines.push('ipv4 = "'+config_escape(ipv4)+'"');
    if(listeners.length){
        var listenerText=[];
        for(var i=0;i<listeners.length;i++)listenerText.push('"'+config_escape(listeners[i])+'"');
        lines.push('listeners = [ '+listenerText.join(', ')+' ]');
    }
    lines.push("");
    lines.push("[network_identity]");
    lines.push('network_name = "'+config_escape(networkName)+'"');
    lines.push('network_secret = "'+config_escape(networkSecret)+'"');
    for(var p=0;p<peers.length;p++){
        lines.push("");
        lines.push("[[peer]]");
        lines.push('uri = "'+config_escape(peers[p])+'"');
    }
    for(var n=0;n<proxyNetworks.length;n++){
        lines.push("");
        lines.push("[[proxy_network]]");
        lines.push('cidr = "'+config_escape(proxyNetworks[n])+'"');
    }
    return lines.join("\n")+"\n";
}
function show_config(){
    E("magictier_export_text").value=build_config_text();
    E("magictier_export_mask").style.display="block";
    E("magictier_export_box").style.display="block";
}
function hide_config(){E("magictier_export_mask").style.display="none";E("magictier_export_box").style.display="none";}
function copy_config_text(){
    var text=E("magictier_export_text").value;
    if(navigator.clipboard&&navigator.clipboard.writeText){
        navigator.clipboard.writeText(text).then(function(){alert("配置文本已复制到剪贴板。");}).catch(function(){copy_config_text_fallback();});
    }else copy_config_text_fallback();
}
function copy_config_text_fallback(){
    var box=E("magictier_export_text");
    box.focus();box.select();
    try{document.execCommand("copy");alert("配置文本已复制到剪贴板。");}catch(e){alert("浏览器不允许自动复制，请手工全选复制。");}
}
function download_config_text(){
    var text=build_config_text();
    var name=(E("magictier_network_name").value||"default").replace(/[^a-zA-Z0-9._-]/g,"_");
    var blob=new Blob([text],{type:"text/plain;charset=utf-8"});
    var url=(window.URL||window.webkitURL).createObjectURL(blob);
    var a=document.createElement("a");
    a.href=url;a.download="magictier-"+name+"-config.txt";
    document.body.appendChild(a);a.click();document.body.removeChild(a);
    setTimeout(function(){(window.URL||window.webkitURL).revokeObjectURL(url);},1000);
}
function config_unescape(v){
    var out="";
    for(var i=0;i<v.length;i++){
        var ch=v.charAt(i);
        if(ch!=="\\"||i+1>=v.length){out+=ch;continue;}
        var next=v.charAt(++i);
        if(next==="n")out+="\n";
        else if(next==="r")out+="\r";
        else if(next==="t")out+="\t";
        else if(next==='"')out+='"';
        else if(next==="\\")out+="\\";
        else out+="\\"+next;
    }
    return out;
}
function toml_value(line){
    var p=line.indexOf("=");
    if(p<0)return "";
    var v=line.substring(p+1).replace(/^\s+|\s+$/g,"");
    if(v.length>=2&&v.charAt(0)==='"'&&v.charAt(v.length-1)==='"'){
        v=config_unescape(v.substring(1,v.length-1));
    }else if(v.length>=2&&v.charAt(0)==="'"&&v.charAt(v.length-1)==="'"){
        v=v.substring(1,v.length-1);
    }
    return v;
}
function import_config_text(text){
    var lines=(text||"").replace(/\r/g,"").split("\n");
    var section="", peers=[], proxy=[], listeners=[];
    var data={hostname:"",instance_name:"",network_name:"",network_secret:"",ipv4:""};
    for(var i=0;i<lines.length;i++){
        var line=lines[i].replace(/^\s+|\s+$/g,"");
        if(!line||line.charAt(0)==="#")continue;
        if(line==="[network_identity]"){section="network_identity";continue;}
        if(line==="[[peer]]"){section="peer";continue;}
        if(line==="[[proxy_network]]"){section="proxy_network";continue;}
        if(line.charAt(0)==="["){section="";continue;}
        var key=line.split("=",1)[0].replace(/^\s+|\s+$/g,"");
        var val=toml_value(line);
        if(section==="network_identity"&&(key==="network_name"||key==="network_secret"))data[key]=val;
        else if(section==="peer"&&key==="uri"&&val)peers.push(val);
        else if(section==="proxy_network"&&key==="cidr"&&val)proxy.push(val);
        else if(!section&&key==="listeners"){
            var m=line.match(/\[(.*)\]/);
            if(m&&m[1])m[1].split(",").forEach(function(x){x=x.replace(/^\s+|\s+$/g,"");if(x.length>=2&&x.charAt(0)==='"'&&x.charAt(x.length-1)==='"')listeners.push(config_unescape(x.substring(1,x.length-1)));});
        }
        else if(!section&&(key==="hostname"||key==="instance_name"||key==="ipv4"))data[key]=val;
    }
    if(!data.network_name&&!data.network_secret&&!data.ipv4&&!data.hostname&&!data.instance_name&&!peers.length&&!proxy.length&&!listeners.length){alert("未识别到有效的 MagicTier 配置，请检查格式。");return false;}
    ["hostname","instance_name","network_name","network_secret","ipv4"].forEach(function(k){if(data[k]!=="")E("magictier_"+k).value=data[k];});
    if(peers.length)E("magictier_peers").value=peers.join(",");
    if(listeners.length)E("magictier_listeners").value=listeners.join(",");
    if(proxy.length)E("magictier_proxy_networks").value=proxy.join(",");
    alert("配置已导入到页面，请检查后点击“保存并应用”。");
    return true;
}
function import_from_textarea(){if(import_config_text(E("magictier_import_text").value))hide_import();}
function import_from_clipboard(){
    if(navigator.clipboard&&navigator.clipboard.readText){
        navigator.clipboard.readText().then(function(t){if(!import_config_text(t))show_import();}).catch(function(){show_import();alert("浏览器未允许直接读取剪贴板，请在文本框中粘贴配置后导入。");});
    }else{
        show_import();
        alert("当前浏览器不支持直接读取剪贴板，请在文本框中粘贴配置后导入。");
    }
}
function reload_Soft_Center(){location.href="/Module_Softcenter.asp";}
</script>
</head>
<body onload="init();">
<div id="TopBanner"></div><div id="Loading" class="popup_bg"></div>
<div id="magictier_log_mask" onclick="hide_log();"></div>
<div id="magictier_log_box"><div style="color:#fff;font-size:16px;margin-bottom:4px;">MagicTier 组网状态日志</div><div style="color:#cfd8dc;font-size:12px;margin-bottom:8px;">仅显示网络名称、连接结果和需要用户处理的信息。</div><textarea id="magictier_log_text" readonly="readonly"></textarea><div style="text-align:center;margin-top:10px;"><input class="button_gen" type="button" onclick="load_log();" value="刷新" />&nbsp;<input class="button_gen" type="button" onclick="clear_log();" value="清空日志" />&nbsp;<input class="button_gen" type="button" onclick="hide_log();" value="关闭" /></div></div>
<div id="magictier_import_mask" onclick="hide_import();"></div>
<div id="magictier_import_box"><div style="color:#fff;font-size:16px;margin-bottom:8px;">粘贴 MagicTier TOML 配置</div><textarea id="magictier_import_text" placeholder="hostname = &quot;my-node&quot;&#10;instance_name = &quot;default&quot;&#10;ipv4 = &quot;10.126.126.50/24&quot;&#10;&#10;[network_identity]&#10;network_name = &quot;company_vpn&quot;&#10;network_secret = &quot;...&quot;"></textarea><div style="text-align:center;margin-top:10px;"><input class="button_gen" type="button" onclick="import_from_textarea();" value="导入到表单" />&nbsp;<input class="button_gen" type="button" onclick="hide_import();" value="关闭" /></div></div>
<div id="magictier_export_mask" onclick="hide_config();"></div>
<div id="magictier_export_box"><div style="color:#fff;font-size:16px;margin-bottom:4px;">MagicTier 当前页面配置</div><div style="color:#ffd54f;font-size:12px;margin-bottom:8px;">注意：文本包含网络密钥；如页面有未保存修改，查看和导出的内容也会包含这些修改，请妥善保管。</div><textarea id="magictier_export_text" readonly="readonly"></textarea><div style="text-align:center;margin-top:10px;"><input class="button_gen" type="button" onclick="copy_config_text();" value="复制文本" />&nbsp;<input class="button_gen" type="button" onclick="download_config_text();" value="导出 TXT" />&nbsp;<input class="button_gen" type="button" onclick="hide_config();" value="关闭" /></div></div>
<table class="content" align="center" cellpadding="0" cellspacing="0"><tr><td width="17">&nbsp;</td><td valign="top" width="202"><div id="mainMenu"></div><div id="subMenu"></div></td><td valign="top"><div id="tabMenu" class="submenuBlock"></div>
<table width="98%" border="0" align="left" cellpadding="0" cellspacing="0"><tr><td align="left" valign="top"><table width="760px" border="0" cellpadding="5" cellspacing="0" class="FormTitle"><tr><td bgcolor="#4D595D" colspan="3" valign="top">
<div>&nbsp;</div><div style="float:left;" class="formfonttitle">MagicTier</div><div style="float:right;width:15px;height:25px;margin-top:10px"><img onclick="reload_Soft_Center();" align="right" style="cursor:pointer;position:absolute;margin-left:-30px;margin-top:-25px;" title="返回软件中心" src="/images/backprev.png" /></div><div style="margin:30px 0 10px 5px;" class="splitLine"></div>
<div class="formfontdesc">MagicTier ARMv7/ARM64 组网插件。当前版本：<span id="magictier_version">-</span></div>
<table style="margin-top:10px;" width="100%" border="1" align="center" cellpadding="4" cellspacing="0" bordercolor="#6b8fa3" class="FormTable"><thead><tr><td colspan="2">运行状态</td></tr></thead>
<tr><th>状态</th><td><span id="run_state">检测中</span>　PID: <span id="run_pid">-</span>　RSS: <span id="run_rss">-</span></td></tr>
<tr><th>操作</th><td><input class="button_gen" type="button" onclick="service_action('start');" value="启动" />&nbsp;<input class="button_gen" type="button" onclick="service_action('stop');" value="停止" />&nbsp;<input class="button_gen" type="button" onclick="service_action('restart');" value="重启" />&nbsp;<input class="button_gen" type="button" onclick="show_log();" value="查看组网日志" /></td></tr></table>
<table style="margin-top:10px;" width="100%" border="1" align="center" cellpadding="4" cellspacing="0" bordercolor="#6b8fa3" class="FormTable"><thead><tr><td colspan="2">运行设置</td></tr></thead>
<tr><th>配置管理</th><td><input class="button_gen" type="button" onclick="import_from_clipboard();" value="从剪贴板导入" />&nbsp;<input class="button_gen" type="button" onclick="show_import();" value="手工粘贴配置" />&nbsp;<input class="button_gen" type="button" onclick="show_config();" value="查看配置" />&nbsp;<input class="button_gen" type="button" onclick="download_config_text();" value="导出文本" /></td></tr>
<tr><th>启用 MagicTier</th><td><input id="magictier_enable" type="checkbox" /></td></tr>
<tr><th>主机名</th><td><input id="magictier_hostname" class="input_ss_table" maxlength="128" placeholder="my-node" /></td></tr>
<tr><th>实例名称</th><td><input id="magictier_instance_name" class="input_ss_table" maxlength="128" placeholder="default" /></td></tr>
<tr><th>网络名称</th><td><input id="magictier_network_name" class="input_ss_table" maxlength="128" /></td></tr>
<tr><th>网络密钥</th><td><input id="magictier_network_secret" type="password" class="input_ss_table" maxlength="256" autocomplete="new-password" /></td></tr>
<tr><th>虚拟 IPv4</th><td><input id="magictier_ipv4" class="input_ss_table" maxlength="64" placeholder="10.144.144.1/24" /></td></tr>
<tr><th>Peer 节点</th><td><input id="magictier_peers" class="input_ss_table" style="width:420px" maxlength="1024" placeholder="tcp://1.2.3.4:11010" /></td></tr>
<tr><th>监听地址</th><td><input id="magictier_listeners" class="input_ss_table" style="width:420px" maxlength="1024" placeholder="tcp://0.0.0.0:11010,udp://0.0.0.0:11010" /></td></tr>
<tr><th>发布子网</th><td><input id="magictier_proxy_networks" class="input_ss_table" style="width:420px" maxlength="512" placeholder="192.168.50.0/24" /></td></tr></table>
<div style="margin-top:15px;text-align:center;"><input class="button_gen" type="button" onclick="save();" value="保存并应用" /></div>
<div style="margin:15px 0 5px 0;" class="formfontdesc">7×24保护：用户组网日志上限约 128KB，内部诊断日志上限约 64KB；默认 RSS 超过 256MB 自动停服并关闭自动启动；进程以较低 CPU 优先级运行。</div>
</td></tr></table></td></tr></table></td></tr></table><div id="footer"></div>
</body></html>
