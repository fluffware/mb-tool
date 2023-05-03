
cargo build --release
md target\windows\mb-tool\web
copy web target\windows\mb-tool\web
copy target\release\mb-tool.exe target\windows\mb-tool
tar -a -c -C target\windows -f target\windows\mb-tool.zip mb-tool\mb-tool.exe mb-tool\web

