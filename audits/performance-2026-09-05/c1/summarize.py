import csv
import math
import pathlib
import statistics

root = pathlib.Path(__file__).resolve().parent / "results"
rows = []
def summarize(backend, phase, repeat, operation, revisions, channels, samples, calls=None, allocated=None):
    times = sorted(samples)
    rows.append(dict(backend=backend, phase=phase, repeat=repeat, operation=operation,
        revisions=revisions, channels=channels, samples=len(times),
        p50_us=statistics.median(times)/1000,
        p95_us=times[math.ceil(.95*len(times))-1]/1000,
        p99_us=times[math.ceil(.99*len(times))-1]/1000,
        operations_per_second=channels*len(times)*1e9/sum(times),
        allocation_calls_per_operation=calls, requested_allocation_bytes_per_operation=allocated))
for phase, label in [("baseline", "baseline-clean"), ("after", "after")]:
    for repeat in range(1,4):
        data=list(csv.DictReader((root/f"memory-{label}-{repeat}.csv").open()))
        for n in [16,128,1024]:
            for c in [1,8]:
                values=[r for r in data if int(r['revisions'])==n and int(r['channels'])==c]
                summarize("memory",phase,repeat,"update",n,c,[int(r['elapsed_ns']) for r in values],
                    statistics.mean(int(r['alloc_calls']) for r in values)/c,
                    statistics.mean(int(r['alloc_bytes']) for r in values)/c)
for phase,label in [("baseline","baseline-v2"),("after","after-final")]:
    for repeat in range(1,4):
        data=[s.split(',') for s in (root/f"sql-{label}-{repeat}.log").read_text().splitlines() if s.startswith(('C1,','C1A,'))]
        for backend in ["postgres","mysql"]:
            for kind,operation in [("C1","update"),("C1A","capped_append")]:
                for n in [16,128,1024]:
                    for c in [1,8]:
                        values=[int(r[5]) for r in data if r[:4]==[kind,backend,str(n),str(c)]]
                        summarize(backend,phase,repeat,operation,n,c,values)
with (root/'summary.csv').open('w') as output:
    writer=csv.DictWriter(output,fieldnames=rows[0].keys());writer.writeheader();writer.writerows(rows)
for backend in ['memory','postgres','mysql']:
    for operation in ['update','capped_append']:
        for c in [1,8]:
            groups=[]
            for phase in ['baseline','after']:
                values=[r['p50_us'] for r in rows if r['backend']==backend and r['phase']==phase and r['operation']==operation and r['revisions']==1024 and r['channels']==c]
                if values:groups.append(f"{min(values):.1f}–{max(values):.1f}")
            if groups:print(backend,operation,c,' | '.join(groups))
