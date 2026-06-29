#!/bin/bash
# Move all classes in index.css into @layer components { ... }
sed -i '' -e '/^\.neo-bg {/i\
@layer components {
' frontend/src/index.css
echo "}" >> frontend/src/index.css
