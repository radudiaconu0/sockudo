"""Run only against the isolated C1 Compose project; reset its fixture databases."""
import pathlib
import subprocess
import sys

binary = str(pathlib.Path(sys.argv[1]).resolve())
label = sys.argv[2]
root = pathlib.Path(__file__).resolve().parent / "results"
for repeat in range(1, 4):
    with (root / f"sql-{label}-{repeat}-reset.log").open("w") as log:
        commands = [
            ["docker", "exec", "sockudo-c1-indexes-postgres-1", "psql", "-U", "c1", "-d", "c1", "-c", "DROP SCHEMA public CASCADE; CREATE SCHEMA public;"],
            ["docker", "exec", "-e", "MYSQL_PWD=c1-local-only", "sockudo-c1-indexes-mysql-1", "mysql", "-uroot", "-e", "DROP DATABASE c1; CREATE DATABASE c1;"],
        ]
        for command in commands:
            subprocess.run(command, stdout=log, stderr=log, check=True)
    with (root / f"sql-{label}-{repeat}.log").open("w") as out, (root / f"sql-{label}-{repeat}.time").open("w") as err:
        subprocess.run(["/usr/bin/time", "-l", binary, "c1_sql_fixed_revisions", "--ignored", "--nocapture"], stdout=out, stderr=err, check=True)
