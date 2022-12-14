import sys
import array as arr
from pathlib import Path
from transformers import pipeline

summarizer = pipeline("summarization", model="facebook/bart-large-cnn")

ARTICLE_PATH = sys.argv[1]
ARTICLE = Path(ARTICLE_PATH).read_text()

# max input length is 4000?
SEGMENT_LIMIT = 4000

SENTENCES = ARTICLE.split(".")

SEGMENTS = []
SEGMENT = ""

for s in SENTENCES:
    if len(SEGMENT) >= SEGMENT_LIMIT:
        SEGMENTS.append(SEGMENT)
        SEGMENT = ""
    SEGMENT += s
    
SEGMENT_SUMMARIES = []
    
for seg in SEGMENTS:
    summary = summarizer(seg, max_length=130, min_length=30, do_sample=False)[0]["summary_text"]
    print(summary)
    SEGMENT_SUMMARIES.append(summary)

JOINED_SEG_SUMMARIES = "".join(SEGMENT_SUMMARIES)

TOKEN_LENGTH = len(JOINED_SEG_SUMMARIES) / 4.7 # magic
MASTER_MIN_LENGTH = min(320, TOKEN_LENGTH)
MASTER_MAX_LENGTH = TOKEN_LENGTH

print("master summary:")
print(summarizer(JOINED_SEG_SUMMARIES, max_length=MASTER_MAX_LENGTH, min_length=MASTER_MIN_LENGTH, do_sample=True)[0]["summary_text"])
