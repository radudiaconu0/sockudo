"""Required C1 verification; retains all diagnostics, even when a gate fails."""
import json
import pathlib
import subprocess

root = pathlib.Path(__file__).resolve().parents[3]
results = pathlib.Path(__file__).resolve().parent / "results"
checks = [
    ("fmt-final", ["cargo", "fmt", "--all"], root),
    ("core-focused", ["cargo", "test", "-p", "sockudo-core"], root),
    ("adapter-focused", ["cargo", "test", "-p", "sockudo-adapter", "--features", "ai-transport"], root),
    ("workspace-test", ["cargo", "test", "--workspace"], root),
    ("workspace-clippy", ["cargo", "clippy", "--workspace", "--all-targets", "--", "-D", "warnings"], root),
    ("minimal-check", ["cargo", "check", "-p", "sockudo", "--no-default-features"], root),
    ("selected-check", ["cargo", "check", "-p", "sockudo", "--no-default-features", "--features", "local,v2,ai-transport,redis,postgres,mysql,push"], root),
    ("full-check", ["cargo", "check", "-p", "sockudo", "--no-default-features", "--features", "full"], root),
    ("docs-types", ["npm", "run", "types:check"], root / "docs"),
    ("docs-build", ["npm", "run", "build"], root / "docs"),
]
summary = []
for name, command, cwd in checks:
    with (results / f"{name}.log").open("w") as output:
        output.write(f"cwd: {cwd}\ncommand: {' '.join(command)}\n")
        output.flush()
        result = subprocess.run(command, cwd=cwd, stdout=output, stderr=output)
    summary.append({"name": name, "command": command, "cwd": str(cwd), "exit_code": result.returncode})
    (results / "verification.json").write_text(json.dumps(summary, indent=2) + "\n")
