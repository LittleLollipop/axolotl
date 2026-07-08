#!/usr/bin/env python3
"""Generate EdgeBlock-Technical-Report.pdf from .md source.
Usage: DYLD_LIBRARY_PATH=/opt/homebrew/lib python3 generate-pdf.py
"""
import subprocess, os

MD = os.path.join(os.path.dirname(__file__), "EdgeBlock-Technical-Report.md")
PDF = os.path.join(os.path.dirname(__file__), "EdgeBlock-Technical-Report.pdf")
HTML = "/tmp/axolotl-gen.html"

# Find CJK font
result = subprocess.run(
    ["fc-list", "--format=%{file}\n", ":family=PingFang SC"],
    capture_output=True, text=True)
paths = [p for p in result.stdout.strip().split('\n') if p]
font_path = paths[0] if paths else "/System/Library/Fonts/STHeiti Medium.ttc"

# Pandoc: MD → HTML
subprocess.run(
    ["pandoc", MD, "-o", HTML, "--standalone", "-M", "document-css=false"],
    check=True)

# Inject CSS with embedded font
with open(HTML) as f:
    content = f.read()

css = f'''<style>
@font-face {{
    font-family: 'CJK';
    src: url('file://{font_path}') format('truetype');
}}
body {{
    font-family: 'CJK', -apple-system, sans-serif;
    font-size: 12pt; line-height: 1.7; color: #222;
}}
pre, code {{ font-family: 'Menlo', monospace; font-size: 9pt; }}
code {{ background: #f5f5f5; padding: 1px 4px; }}
pre {{ background: #f5f5f5; padding: 12px; white-space: pre-wrap; }}
pre code {{ background: none; padding: 0; }}
table {{ border-collapse: collapse; width: 100%; margin: 12px 0; font-size: 10pt; }}
th, td {{ border: 1px solid #999; padding: 6px 10px; text-align: left; }}
th {{ background: #e8e8e8; font-weight: bold; }}
h1 {{ font-size: 22pt; border-bottom: 2px solid #444; padding-bottom: 6px; }}
h2 {{ font-size: 16pt; border-bottom: 1px solid #aaa; padding-bottom: 4px; margin-top: 32px; }}
h3 {{ font-size: 13pt; margin-top: 24px; }}
blockquote {{ border-left: 4px solid #ccc; padding: 8px 16px; color: #555; margin: 16px 0; background: #fafafa; }}
</style>'''

content = content.replace("<head>", "<head>\n<meta charset=\"utf-8\">\n" + css)
with open(HTML, "w") as f:
    f.write(content)

# Weasyprint: HTML → PDF
from weasyprint import HTML as WH
WH(filename=HTML).write_pdf(PDF)
os.remove(HTML)

print(f"PDF generated: {PDF} ({os.path.getsize(PDF) / 1024:.0f} KB)")
