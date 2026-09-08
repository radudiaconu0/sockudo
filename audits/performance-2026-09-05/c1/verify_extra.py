import json
import os
import pathlib
import subprocess
root=pathlib.Path(__file__).resolve().parents[3]
results=pathlib.Path(__file__).resolve().parent/'results'
features='local,v2,ai-transport,ably-compat,redis,postgres,mysql,push'
env=os.environ.copy();env['REDIS_URL']='redis://127.0.0.1:25463/'
checks=[
 ('selected-http-tests',['cargo','test','-p','sockudo','--no-default-features','--features',features,'versioned_messages']),
 ('sql-final-correctness',['cargo','test','-p','sockudo','--no-default-features','--features',features,'c1_sql_append_caps','--','--ignored','--nocapture']),
 ('selected-clippy',['cargo','clippy','-p','sockudo','--all-targets','--no-default-features','--features',features,'--','-D','warnings']),
 ('live-build',['cargo','build','-p','sockudo','--no-default-features','--features',features]),
]
summary=[]
for name,command in checks:
 with (results/f'{name}.log').open('w') as out:
  out.write('command: '+' '.join(command)+'\nenv: REDIS_URL=redis://127.0.0.1:25463/\n');out.flush()
  result=subprocess.run(command,cwd=root,env=env,stdout=out,stderr=out)
 summary.append({'name':name,'command':command,'exit_code':result.returncode})
 (results/'verification-extra.json').write_text(json.dumps(summary,indent=2)+'\n')
