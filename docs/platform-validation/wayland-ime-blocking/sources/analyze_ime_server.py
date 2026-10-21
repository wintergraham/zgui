import json,re,pathlib,sys
root=pathlib.Path(sys.argv[1]); result={}
for path in sorted(root.glob('*/*/sway.log')):
    counters={}; pending={}; rows=[]
    for number,line in enumerate(path.read_text().splitlines(),1):
        match=re.search(r'\[\s*([\d.]+)\]\s*(-> )?zwp_input_method_v2#(\d+)\.(\w+)\((.*)\)',line)
        if not match: continue
        stamp,direction,obj,method,args=match.groups()
        if direction and method=='done': counters[obj]=counters.get(obj,0)+1
        elif not direction and method in ('set_preedit_string','commit_string'):
            pending.setdefault(obj,{})[method]=json.loads(re.match(r'("(?:[^"\\]|\\.)*")',args)[1])
        elif not direction and method=='commit':
            serial=int(args); current=counters.get(obj,0)
            rows.append(dict(line=number,timestamp_ms=float(stamp),object_id=obj,serial=serial,issued_done_count=current,serial_matches=serial==current,**pending.pop(obj,{})))
    result[str(path.relative_to(root))]=dict(transactions=rows,serial_mismatches=sum(not row['serial_matches'] for row in rows))
(root/'server-serial-analysis.json').write_text(json.dumps(result,indent=2,ensure_ascii=False)+'\n')
print(json.dumps({p:v['serial_mismatches'] for p,v in result.items()},indent=2))
