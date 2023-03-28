class AreaUpdater {
    value_map = new RangeDict();
    mb_values = [];
    constructor(parent, send) {
        this.send = send;

        var values = parent.getElementsByClassName("mb_value");
        for (let v of values) {
            let addr_low = parseInt(v.getAttributeNS(MB_NS, "addr-low"));
            let addr_high = parseInt(v.getAttributeNS(MB_NS, "addr-high"));
            console.log(addr_low);
            console.log(addr_high);
            this.value_map.insert(addr_low, addr_high + 1, v);
            let inp = v;
            let mb_values = this.mb_values;
            let updater = this;
            v.addEventListener("change", function(e) {
                let low = inp.getAttributeNS(MB_NS, "bit-low");
                let high = inp.getAttributeNS(MB_NS, "bit-high");
                let disp = inp.getAttributeNS(MB_NS, "value-type");
                switch (disp) {
                    case "integer":
                        {

                            let value;
                            if (inp.type == "checkbox") {
                                value = BigInt(e.target.checked ? 1 : 0);
                            } else {
                                try {
                                    value = BigInt(e.target.value);
                                } catch {
                                    value = Number(e.target.value);
                                }
                            }
                            if (low != null && high != null && addr_low == addr_high) {
                                let old_value = mb_values[addr_low] || 0;
                                let mask = ((1 << (high - low + 1)) - 1) << low;
                                value = BigInt((old_value & ~mask) | (Number(value) << low) & mask);
                            }
                            let scale = inp.getAttributeNS(MB_NS, "scale");
                            if (scale != null && scale != 1) {
                                value = Math.round(Number(value) * scale);

                            }
                            if (typeof value == "number") value = Math.round(value);
                            let byte_swap = inp.getAttributeNS(MB_NS, "byte-order") == "little";
                            let word_order = inp.getAttributeNS(MB_NS, "word-order");
                            value = BigInt(value);
                            console.log("Changed value: " + value);
                            if (word_order == "little") {
                                for (let a = addr_low; a <= addr_high; a++) {
                                    mb_values[a] = Number(value & BigInt(0xffff));
                                    value >>= BigInt(16);
                                }
                            } else {
                                for (let a = addr_high; a >= addr_low; a--) {
                                    mb_values[a] = Number(value & BigInt(0xffff));
                                    value >>= BigInt(16);
                                }
                            }


                        }
                        break;
                    case "string":
                        {
                            let fill = inp.getAttributeNS(MB_NS, "fill")
                            let low_first = inp.getAttributeNS(MB_NS, "byte-order") == "little";
                            let encoder = new TextEncoder("utf-8");
                            let byte_length = (addr_high - addr_low) * 2 + 2;
                            let bytes = new Uint8Array(byte_length);
                            bytes.fill(fill);
                            bytes.set(encoder.encode(inp.value).slice(0, byte_length));
                            for (let a = addr_low; a <= addr_high; a++) {
                                let c = (a - addr_low) * 2;
                                if (low_first) {
                                    mb_values[a] = bytes[c] | (bytes[c + 1] << 8);
                                } else {
                                    mb_values[a] = bytes[c + 1] | (bytes[c] << 8);
                                }
                            }
                        }
                        break;
                }
                updater.send({
                    start: addr_low,
                    regs: mb_values.slice(addr_low, addr_high + 1)
                });
                updater.update_range(addr_low, addr_high);
            });
            v.addEventListener("blur", function(e) {
                updater.update_range(addr_low, addr_high);
            });

        }
    }

    update_values(addr, v) {
        Array.prototype.splice.apply(this.mb_values, [addr, v.length].concat(v));
        this.update_range(addr, addr + v.length - 1)
    }

    static swap16(v) {
        return ((v >> 8) & 0xff)((v & 0xff) << 8);
    }
    start_int(addr, swap, signed) {
        let word = this.mb_values[addr];
        if (swap) word = swap16(word);
        if (signed && word >= 32768) word -= 65536;
        return BigInt(word);
    }
    acc_int(sum, addr, swap) {
        let word = this.mb_values[addr];
        if (swap) word = swap16(word);

        return sum * BigInt(65536) + BigInt(word);
    }
    update_range(addr_low, addr_high) {
        let updates = this.value_map.overlapping(addr_low, addr_high + 1);

        for (let update of updates) {
            let inp = update.value;
            if (!(inp === document.activeElement)) {
                let addr_low = parseInt(inp.getAttributeNS(MB_NS, "addr-low"));
                let addr_high = parseInt(inp.getAttributeNS(MB_NS, "addr-high"));
                let value_type = inp.getAttributeNS(MB_NS, "value-type") || "integer";
                switch (value_type) {
                    case "integer":
                        {
                            let signed = inp.getAttributeNS(MB_NS, "sign") == "signed";
                            let byte_swap = inp.getAttributeNS(MB_NS, "byte-order") == "little";
                            let word_order = inp.getAttributeNS(MB_NS, "word-order");
                            let value;
                            if (word_order == "little") {
                                value = this.start_int(addr_high, byte_swap, signed);
                                for (let a = addr_high - 1; a >= addr_low; a--) {
                                    value = this.acc_int(value, a, byte_swap);
                                }
                            } else {
                                value = this.start_int(addr_low, byte_swap, signed);
                                for (let a = addr_low + 1; a <= addr_high; a++) {
                                    value = this.acc_int(value, a, byte_swap);
                                }
                            }
                            let low = inp.getAttributeNS(MB_NS, "bit-low");
                            let high = inp.getAttributeNS(MB_NS, "bit-high");


                            if (low != null && high != null) {
                                value = (value >> BigInt(low)) & BigInt((1 << (high - low + 1)) - 1);
                            }
                            let scale = inp.getAttributeNS(MB_NS, "scale");
                            if (scale != null && scale != 1) {
                                value = Number(value) / scale;
                            }
                            if (inp.localName == "input") {
                                if (inp.type == "checkbox") {
                                    inp.checked = Number(value) > 0;
                                } else {
                                    if (typeof(value) == "bigint") {
                                        let base = inp.getAttributeNS(MB_NS, "base") || 10;
                                        if (base == 16) {
                                            inp.value = "0x" + value.toString(16);
                                        } else if (base == 2) {
                                            inp.value = "0b" + value.toString(2);
                                        } else {
                                            inp.value = value;
                                        }

                                    } else {
                                        inp.value = value;
                                    }
                                }
                            }
                        }
                        break;
                    case "float":
                        breaak;
                    case "string":
                        let bytes = [];
                        let fill = inp.getAttributeNS(MB_NS, "fill")
                        let low_first = inp.getAttributeNS(MB_NS, "byte-order") == "little";
                        let end = null;
                        for (let a = addr_low; a <= addr_high; a++) {

                            let w = this.mb_values[a];
                            let first;
                            let second;
                            if (low_first) {
                                first = w & 0xff;
                                second(w >> 8) & 0xff;
                            } else {
                                first = (w >> 8) & 0xff;
                                second = w & 0xff;
                            }
                            if (end == null) {
                                if (first == fill) {
                                    end = (a - addr_low) * 2;
                                } else {
                                    if (second == fill) {
                                        end = (a - addr_low) * 2 + 1;
                                    }
                                }
                            }
                            bytes.push(first);
                            bytes.push(second);
                        }
                        if (end == null) {
                            end = (addr_high - addr_low) * 2 + 2;
                        }
                        let decoder = new TextDecoder("utf-8");
                        let text = decoder.decode(new Uint8Array(bytes.slice(0, end)));
                        inp.value = text;
                        break;
                    default:
                        console.log("Unknown value type " + value_type);
                }
            }
        }
    }
}


function socket_uri() {
    var loc = window.location,
        new_uri;
    if (loc.protocol === "https:") {
        new_uri = "wss:";
    } else {
        new_uri = "ws:";
    }
    new_uri += "//" + loc.host;
    new_uri += "/socket/";
    return new_uri;
}
const MB_NS = "http://www.elektro-kapsel.se/xml/mb-tool";

function setup() {
    ws = new WebSocket(socket_uri());
    var holding_regs_elems = document.getElementById("holding_registers");
    let holding_regs = new AreaUpdater(holding_regs_elems,
        function(data) {
            ws.send(JSON.stringify({ UpdateHoldingRegs: data }))
        });

    var input_regs_elems = document.getElementById("input_registers");
    let input_regs = new AreaUpdater(input_regs_elems,
        function(data) {
            ws.send(JSON.stringify({ UpdateInputRegs: data }))
        });


    ws.onmessage = (msg) => {
        let cmd = JSON.parse(msg.data);
        let holding_registers = cmd.UpdateHoldingRegs;
        if (holding_registers) {
            holding_regs.update_values(holding_registers.start, holding_registers.regs);
        }
        let input_registers = cmd.UpdateInputRegs;
        if (input_registers) {
            input_regs.update_values(input_registers.start, input_registers.regs);
        }

    };

    ws.onopen = () => {
        ws.send(JSON.stringify({ RequestHoldingRegs: { start: 0, length: 256 } }))
        ws.send(JSON.stringify({ RequestInputRegs: { start: 0, length: 256 } }))

    };
}