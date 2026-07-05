import os

with open('Cargo.toml', 'r', encoding='utf-8') as f:
    lines = f.readlines()

new_lines = []
for line in lines:
    if 'tokio-tungstenite = ' in line or 'futures-util = ' in line:
        continue
    new_lines.append(line)
    if '[dependencies]' in line:
        new_lines.append('tokio-tungstenite = "0.21.0"\n')
        new_lines.append('futures-util = "0.3.30"\n')

with open('Cargo.toml', 'w', encoding='utf-8') as f:
    f.writelines(new_lines)
