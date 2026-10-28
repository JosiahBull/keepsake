# Default lists

The server adds these lists to Josiah's and Ansam's libraries when `DEFAULT_LISTS_DIR`
names this directory. The container image and `pnpm dev` set it. Each list is a
directory named after its `id`, holding `list.json` and the files it names.

```json
{
  "id": "nz-birds",
  "title": "New Zealand birds",
  "items": [
    {
      "key": "tui-photo-1",
      "prompt": "Which bird is this?",
      "answer": "Tūī",
      "context": "Identification cues, then the photo's credit.",
      "source": "https://www.nzbirdsonline.org.nz/species/tui",
      "tags": ["nz-birds", "nz-birds-forest"],
      "practices": [],
      "media": [
        {
          "file": "media/tui-adult-1.webp",
          "alt": "What the image shows, without naming the bird",
          "ai_generated": false,
          "credit": "Photographer, licence, source page"
        }
      ]
    }
  ]
}
```

Items take the fields of a new fact, with media named by file instead of uploaded. The
server checks every list when it starts and refuses to start if one is malformed.

- **Keys are permanent.** Each library records the keys it has received and adds an item
  once. Editing an item later does not change libraries that already have it. To replace
  one, give the new version a new key.
- **Every file has a credit**: who made it, its licence and where it came from. The
  item's context names the author and licence, so learners see them after answering.
- **Mark generated images and clips** with `"ai_generated": true`. Keepsake labels them
  wherever the question is shown. Generated images also carry a visible watermark,
  applied with `uv run scripts/watermark-ai-image.py IN OUT` before they are added here.
- **Describe images without the answer.** The description is read to screen-reader
  users before they answer.

The first start after a list grows converts its new images and clips. Both lists took
under a minute on a recent laptop; a small server takes longer. The server answers
requests meanwhile.

## Checking the lists

Each identification was checked against NZ Birds Online or the NZ Plant Conservation
Network, and each photo was looked at. The calls were trimmed and checked from their
spectrograms and their recordists' labels rather than by ear, so listen to each once,
starting with `titipounamu-call-1` and `ruru-juvenile-call-1`. One kiwi photo shows a
Stewart Island tokoeka, which its card says. No image or clip in these lists is
AI-generated.
