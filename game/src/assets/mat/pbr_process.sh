PREFIX="$1"
OUTPUT="$2"
if [ -z "$PREFIX" ]; then
    echo Specify input prefix
    exit 1
fi
if [ -z "$OUTPUT" ]; then
    echo Specify output prefix
    exit 1
fi

convert "${PREFIX}_Color.jpg" -resize 1024x1024 "$OUTPUT/albedo.png"
convert "${PREFIX}_NormalGL.jpg" -resize 1024x1024 "$OUTPUT/normal.png"

# Create roughess/metalness texture
convert \
  -size 1024x1024 xc:black \
  \( "${PREFIX}_Roughness.jpg" -resize 1024x1024 \) -compose CopyGreen -composite \
  \( "${PREFIX}_Metalness.jpg" -resize 1024x1024 \) -compose CopyBlue -composite \
  "$OUTPUT/pbr.png"