import os

content = open('server.js', 'r', encoding='utf-8').read()
content = content.replace(r'\`', '`')
content = content.replace(r'\$', '$')
open('server.js', 'w', encoding='utf-8').write(content)
