cargo build --target x86_64-unknown-uefi;
cp target/x86_64-unknown-uefi/debug/minesweeper.efi esp/efi/boot/bootx64.efi
qemu-system-x86_64 -enable-kvm \
    -drive if=pflash,format=raw,readonly=on,file=OVMF_CODE_4M.fd \
    -drive if=pflash,format=raw,readonly=on,file=OVMF_VARS_4M.fd \
    -drive format=raw,file=fat:rw:esp
