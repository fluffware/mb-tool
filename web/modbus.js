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
                if (disp == "integer") {

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

                    updater.send({
                        start: addr_low,
                        regs: mb_values.slice(addr_low, addr_high + 1)
                    });
                    updater.update_range(addr_low, addr_high, value);
                }
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
                            if (scale != null) {
                                value = Number(value) / scale;
                            }
                            if (inp.localName == "input") {
                                if (inp.type == "checkbox") {
                                    inp.checked = Number(value) > 0;
                                } else {
                                    inp.value = value;
                                }
                            }
                        }
                        break;
                    case "float":
                        breaak;
                    case "string":
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