---
"lounge": minor
---

Early start on a cold boot (executable Home method): a small root service registered in /var/luna-service2 and a compositor key filter mount Lounge about 18 s into the boot and open Home at once, instead of after the Homebrew hook (~40 s). If any part fails, the hook takes over as before; turning Lounge as Home off removes every part.
