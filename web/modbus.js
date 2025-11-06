function swap_bytes(value, swap) {
    if (swap) {
        return ((value & 0x00ff) << 8) | ((value & 0xff00) >> 8);
    }
    return value;
}


function u16_to_dataview(values, first, last, byte_le, word_le) {
    let l = last - first +1;
    let b = new ArrayBuffer(l * 2);
    let v = new DataView(b);
    if (word_le) {
	for (let i = 0; i < l; i++) {
	    v.setUint16((l-i-1)*2, values[first + i], byte_le);
	}
    } else {
	for (let i = 0; i < l; i++) {
	    v.setUint16(i*2, values[first + i], byte_le);
	}
    }
    return v;
}

function dataview_to_u16(dv, values, first, last, byte_le, word_le) {
    let l = last - first +1;
    if (word_le) {
	for (let i = 0; i < l; i++) {
	    values[first + i] = dv.getUint16((l-i-1)*2,  byte_le);
	}
    } else {
	for (let i = 0; i < l; i++) {
	    values[first + i] = dv.getUint16(i*2, byte_le);
	}
    }
}


	    
function start_int(mb_values, addr, swap, signed) {
    let word = mb_values[addr] || 0;
        if (swap) word = swap16(word);
    if (signed && word >= 32768) word -= 65536;
    return BigInt(word);
}
    
function acc_int(mb_values, sum, addr, swap) {
    let word = mb_values[addr] || 0;
    if (swap) word = swap16(word);

    return sum * BigInt(65536) + BigInt(word);
}
    

class RegisterAreaUpdater {
    devices={};
    //value_map = new RangeDict();
    //mb_values = [];
    focusedElement = null;
    get_device(unit_addr) {
	let dev = this.devices[unit_addr];
	if (dev == null) {
	    dev = {value_map: new RangeDict(), mb_values: []};
	    this.devices[unit_addr] = dev;
	}
	return dev;
    }
    constructor(parent, send) {
        this.send = send;
        {
            let a = new Uint32Array([0x12345678]);
            this.nativeBigEndian = new Uint8Array(a.buffer, a.byteOffset, a.byteLength)[0] == 0x12;
        }

        var values = parent.getElementsByClassName("mb_value");
        for (let v of values) {
            let addr_low = parseInt(v.getAttributeNS(MB_NS, "addr-low"));
            let addr_high = parseInt(v.getAttributeNS(MB_NS, "addr-high"));
            let unit_addr = parseInt(v.getAttributeNS(MB_NS, "unit-addr"));
	    let dev = this.get_device(unit_addr);
            dev.value_map.insert(addr_low, addr_high + 1, v);
            let inp = v;
            let mb_values = dev.mb_values;
            let updater = this;
            v.addEventListener("change", function (e) {
                let low = inp.getAttributeNS(MB_NS, "bit-low");
                let high = inp.getAttributeNS(MB_NS, "bit-high");
                let disp = inp.getAttributeNS(MB_NS, "value-type") || "integer";
                switch (disp) {
                    case "integer":
                        {

                            let value;
                            if (inp.type == "checkbox") {
                                value = BigInt(e.target.checked ? 1 : 0);
                            } else {
                                try {
                                    let s = e.target.value;
                                    let neg = false;
                                    if (s.startsWith("-")) {
                                        neg = true;
                                        s = s.slice(1);
                                    }
                                    value = BigInt(s);
                                    if (neg) {
                                        value = -value;
                                    }
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
			    let byte_le =inp.getAttributeNS(MB_NS, "byte-order") == "little";
                            let byte_swap = this.nativeBigEndian == byte_le; 
                            let word_order = inp.getAttributeNS(MB_NS, "word-order");
                            value = BigInt(value);
                            console.log("Changed value: " + value);
                            if (word_order == "little") {
                                for (let a = addr_low; a <= addr_high; a++) {
                                    mb_values[a] = swap_bytes(Number(value & BigInt(0xffff)), byte_swap);
                                    value >>= BigInt(16);
                                }
                            } else {
                                for (let a = addr_high; a >= addr_low; a--) {
                                    mb_values[a] = swap_bytes(Number(value & BigInt(0xffff)), byte_swap);
                                    value >>= BigInt(16);
                                }
                            }


                        }
                        break;
                    case "float":
                        {
                            let byte_le = inp.getAttributeNS(MB_NS, "byte-order") == "little";
			    let byte_swap = this.nativeBigEndian == byte_le;
                            let word_le = inp.getAttributeNS(MB_NS, "word-order") == "little";
                            let word_count = addr_high - addr_low + 1;
                            let value = Number(e.target.value);
                            if (word_count == 2) {
				let dv = new DataView(new ArrayBuffer(4));
				dv.setFloat32(0,value);
				dataview_to_u16(dv, mb_values, addr_low, addr_high, byte_le, word_le);
                            } else if (word_count == 4) {
				let dv = new DataView(new ArrayBuffer(8));
				dv.setFloat64(0,value);
				dataview_to_u16(dv, mb_values, addr_low, addr_high, byte_le, word_le);
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
		    unit_addr: unit_addr,
                    start: addr_low,
                    regs: mb_values.slice(addr_low, addr_high + 1)
                });
                updater.update_range(unit_addr, addr_low, addr_high);
            });
            v.addEventListener("focus", function (e) {
                updater.focusedElement = inp;
            });
            v.addEventListener("blur", function (e) {
                updater.focusedElement = null;
                updater.update_range(unit_addr, addr_low, addr_high);
            });

        }
    }

    update_values(unit_addr, addr, v) {
	let dev = this.get_device(unit_addr);
	while(addr > dev.mb_values.length) dev.mb_values.push(0);
        dev.mb_values.splice(addr, v.length, ...v);
        this.update_range(unit_addr, addr, addr + v.length - 1)
    }

    static swap16(v) {
        return ((v >> 8) & 0xff)((v & 0xff) << 8);
    }
    
    update_range(unit_addr, addr_low, addr_high) {
	let dev = this.get_device(unit_addr);
        let updates = dev.value_map.overlapping(addr_low, addr_high + 1);

        for (let update of updates) {
            let inp = update.value;
            if (!(inp === this.focusedElement)) {
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
                                value = start_int(dev.mb_values, addr_high, byte_swap, signed);
                                for (let a = addr_high - 1; a >= addr_low; a--) {
                                    value = acc_int(dev.mb_values, value, a, byte_swap);
                                }
                            } else {
                                value = start_int(dev.mb_values, addr_low, byte_swap, signed);
                                for (let a = addr_low + 1; a <= addr_high; a++) {
                                    value = acc_int(dev.mb_values, value, a, byte_swap);
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
                                    if (typeof (value) == "bigint") {
                                        let radix = inp.getAttributeNS(MB_NS, "radix") || 10;
                                        let sign = "";
                                        if (value < 0n) {
                                            sign = "-";
                                            value = -value;
                                        }
                                        if (radix == 16) {
                                            inp.value = sign + "0x" + value.toString(16);
                                        } else if (radix == 2) {
                                            inp.value = sign + "0b" + value.toString(2);
                                        } else {
                                            inp.value = sign + value;
                                        }

                                    } else {
                                        inp.value = value;
                                    }
                                }
                            }
			    else if (inp.localName == "select") {
				 inp.value = Number(value);
			    }
                        }
                        break;
                    case "float":
                        {
                            let byte_le = inp.getAttributeNS(MB_NS, "byte-order") == "little";
                            let word_le = inp.getAttributeNS(MB_NS, "word-order") == "little";
			    let view = u16_to_dataview(dev.mb_values, addr_low, addr_high, byte_le, word_le)
                            let word_count = addr_high - addr_low + 1;
                            if (word_count == 2) {
                                inp.value = view.getFloat32(0);
                            } else if (word_count == 4) {
                                inp.value = view.getFloat64(0);
                            }
                        }
                        break;
                    case "string":
                        let bytes = [];
                        let fill = inp.getAttributeNS(MB_NS, "fill")
                        let low_first = inp.getAttributeNS(MB_NS, "byte-order") == "little";
                        let end = null;
                        for (let a = addr_low; a <= addr_high; a++) {

                            let w = dev.mb_values[a];
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

class BitAreaUpdater {
  
    focusedElement = null;
    devices={};
    
    focusedElement = null;
    get_device(unit_addr) {
	let dev = this.devices[unit_addr];
	if (dev == null) {
	    dev = {value_map: new RangeDict(), mb_values: []};
	    this.devices[unit_addr] = dev;
	}
	return dev;
    }
    constructor(parent, send) {
        this.send = send;

        var values = parent.getElementsByClassName("mb_value");
        for (let v of values) {
            let addr = parseInt(v.getAttributeNS(MB_NS, "addr"));
	    let unit_addr = parseInt(v.getAttributeNS(MB_NS, "unit-addr"));
	    let dev = this.get_device(unit_addr);
            dev.value_map.insert(addr, addr + 1, v);
            let inp = v;
            let mb_values = dev.mb_values;
            let updater = this;
            v.addEventListener("change", function (e) {
                if (inp.type == "checkbox") {
                    mb_values[addr] = e.target.checked;
                }                
                updater.send({
		    unit_addr: unit_addr,
                    start: addr,
                    regs: mb_values.slice(addr, addr + 1)
                });
                updater.update_range(unit_addr, addr, addr+1);
            });
            v.addEventListener("focus", function (e) {
                updater.focusedElement = inp;
            });
            v.addEventListener("blur", function (e) {
                updater.focusedElement = null;
                updater.update_range(unit_addr, addr, addr+1);
            });

        }
    }

    update_values(unit_addr, addr, v) {
	let dev = this.get_device(unit_addr);
	while(addr > dev.mb_values.length) dev.mb_values.push(0);
        dev.mb_values.splice(addr, v.length, ...v);
        this.update_range(unit_addr, addr, addr + v.length - 1)
    }

 
    update_range(unit_addr, addr_low, addr_high) {
	let dev = this.get_device(unit_addr);
        let updates = dev.value_map.overlapping(addr_low, addr_high + 1);

        for (let update of updates) {
            let inp = update.value;
            if (!(inp === this.focusedElement)) {
                let addr= parseInt(inp.getAttributeNS(MB_NS, "addr"));
                if (inp.localName == "input") {
                    if (inp.type == "checkbox") {
                        inp.checked = Number(dev.mb_values[addr]);
                    }
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
    let holding_regs = null;
    if (holding_regs_elems) {
	holding_regs = new RegisterAreaUpdater(
	    holding_regs_elems,
	    function (data) {
		ws.send(JSON.stringify({ UpdateHoldingRegs: data }))
	    });
    }
    
    var input_regs_elems = document.getElementById("input_registers");
    let input_regs = null;
    if (input_regs_elems) {
	input_regs = new RegisterAreaUpdater(
	    input_regs_elems,
	    function (data) {
		ws.send(JSON.stringify({ UpdateInputRegs: data }))
            });
    }
    
    var coils_elems = document.getElementById("coils");
    let coils = null;
    if (coils_elems) {
	coils = new BitAreaUpdater(
	    coils_elems,
	    function (data) {
		ws.send(JSON.stringify({ UpdateCoils: data }))
	    });
    }
    
    var discrete_inputs_elems = document.getElementById("discrete_inputs");
    let discrete_inputs = null;
    if (discrete_inputs_elems) {
	discrete_inputs = new BitAreaUpdater(
	    discrete_inputs_elems,
	    function (data) {
		ws.send(JSON.stringify({ UpdateDiscreteInputs: data }))
	    });
    }
    let echo_count = 0;
    setInterval(function() {
	ws.send(JSON.stringify({Echo: echo_count}));
	echo_count++;
    }, 1000);

    // Collapse groups
    for (g of document.getElementsByClassName("group_block")) {
	let header = g.querySelector(".group_header");
	let indicator = header.querySelector(".group_indicator");
	indicator.setAttribute("src", "/collapsed.svg");
	let body =  g.querySelector(".group_body");
	body.style.display = "none";
	header.addEventListener("click", function() {
	     if (body.style.display === "block") {
	    	 indicator.setAttribute("src", "/collapsed.svg");
		 body.style.display = "none";
	     } else {
	    	 indicator.setAttribute("src", "/expanded.svg");
		 body.style.display = "block";
	     }
	});
    }
    let heart_beat = document.getElementById("heart_beat");
    
    // Receive updates
    ws.onmessage = (msg) => {
        let cmd = JSON.parse(msg.data);
        let holding_registers = cmd.UpdateHoldingRegs;
        if (holding_registers && holding_regs) {
            holding_regs.update_values(holding_registers.unit_addr,
				       holding_registers.start, holding_registers.regs);
        }
        let input_registers = cmd.UpdateInputRegs;
        if (input_registers && input_regs) {
            input_regs.update_values(input_registers.unit_addr, 
				     input_registers.start, input_registers.regs);
        }
	let update_coils = cmd.UpdateCoils;
        if (update_coils && coils) {
            coils.update_values(update_coils.unit_addr,
				update_coils.start, update_coils.regs);
        }
	let update_discrete_inputs = cmd.UpdateDiscreteInputs;
        if (update_discrete_inputs && discrete_inputs) {
            discrete_inputs.update_values(update_discrete_inputs.unit_addr,
					  update_discrete_inputs.start, update_discrete_inputs.regs);
        }

	let echo_reply = cmd.Echo;
	if (echo_reply != null) {
	    if (heart_beat) {
		heart_beat.classList.add("pulse");
		setTimeout(function () {
		    heart_beat.classList.add("remove");
		}, 500);
	    }
	}

	let unit_addresses = cmd.ListUnitAddresses;
        if (unit_addresses) {
	    console.log("Units: "+unit_addresses);
	    for (u of unit_addresses) {
		ws.send(JSON.stringify({ RequestHoldingRegs: {unit_addr: u,
							      start: 0, length: 32768 } }))
		ws.send(JSON.stringify({ RequestHoldingRegs: {unit_addr: u,
							      start: 32768, length: 32768 } }))
		ws.send(JSON.stringify({ RequestInputRegs: {unit_addr: u,
							    start: 0, length: 32768 } }))
		ws.send(JSON.stringify({ RequestInputRegs: {unit_addr: u,
							    start: 32768, length: 32768 } }))
		
		ws.send(JSON.stringify({ RequestCoils: {unit_addr: u,
							start: 0, length: 32768 } }))
		ws.send(JSON.stringify({ RequestCoils: {unit_addr: u,
							start: 32768, length: 32768 } }))
		
		ws.send(JSON.stringify({ RequestDiscreteInputs: {unit_addr: u,
								 start: 0, length: 32768 } }))
		ws.send(JSON.stringify({ RequestDiscreteInputs: {unit_addr: u,
								 start: 32768, length: 32768 } }))
	    }
	}
    };
    ws.onopen = () => {
	ws.send(JSON.stringify({ ListUnitAddresses: [] }))
    };
	/*
    ws.onopen = () => {
        ws.send(JSON.stringify({ RequestHoldingRegs: { start: 0, length: 32768 } }))
        ws.send(JSON.stringify({ RequestHoldingRegs: { start: 32768, length: 32768 } }))
        ws.send(JSON.stringify({ RequestInputRegs: { start: 0, length: 32768 } }))
        ws.send(JSON.stringify({ RequestInputRegs: { start: 32768, length: 32768 } }))

	ws.send(JSON.stringify({ RequestCoils: { start: 0, length: 32768 } }))
        ws.send(JSON.stringify({ RequestCoils: { start: 32768, length: 32768 } }))
	
	ws.send(JSON.stringify({ RequestDiscreteInputs: { start: 0, length: 32768 } }))
        ws.send(JSON.stringify({ RequestDiscreteInputs: { start: 32768, length: 32768 } }))
    };*/
}
