

class AreaUpdater
{
    value_map = {};
    mb_values = {};
    constructor(parent, send) {
	this.send = send;
	
	var values = parent.getElementsByClassName("mb_value");
	for (let v of values) {
            let addr = parseInt(v.getAttributeNS(MB_NS, "addr"));
            console.log(addr);
            if (this.value_map[addr]) {
		this.value_map[addr].push(v);
            } else {
		this.value_map[addr] = [v];
            }
	    let inp = v;
	    let mb_values = this.mb_values;
	    let updater = this;
            v.addEventListener("change", function(e) {
		let low = inp.getAttributeNS(MB_NS, "bit_low");
		let high = inp.getAttributeNS(MB_NS, "bit_high");
		let value;
		if (inp.type == "checkbox") {
		    value = e.target.checked ? 1:0;
		} else {
		    value = parseFloat(e.target.value);
		} 
		if (low != null && high != null) {
		    let old_value = mb_values[addr] || 0;
		    let mask = ((1 << (high - low + 1)) - 1) << low;
                    value = (old_value & ~mask) | (value << low) & mask;
		}
		let scale = inp.getAttributeNS(MB_NS, "scale");
		if (scale == null) {
		    scale = 1;
		}
		value = Math.round(value * scale);

		updater.send({
		    start: addr, regs: [value]
		});
		updater.update_value(addr, value);
            });
	    v.addEventListener("blur", function(e) {
		let value = mb_values[addr];
		if (value != undefined) {
		    updater.update_value(addr, value);
		}
	    });
	    
	}
    }

    update_value(a, v) {
	let inps = this.value_map[a];
	this.mb_values[a] = v;
	if (inps) {
            for (let inp of inps) {
		if (!(inp === document.activeElement)) {
		    let low = inp.getAttributeNS(MB_NS, "bit_low");
		    let high = inp.getAttributeNS(MB_NS, "bit_high");
		    let value;
		    if (low != null && high != null) {
			value = (v >> low) & ((1 << (high - low + 1)) - 1);
		    } else {
			value = v;
		    }
		    let scale = inp.getAttributeNS(MB_NS, "scale");
		    if (scale != null) {
			value /= scale;
		    }
		    if (inp.localName == "input") {
			if (inp.type == "checkbox") {
			    inp.checked = value;
			} else {
			    inp.value = value;
			}
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
    let holding_regs = new AreaUpdater(holding_regs_elems,
				       function(data) {
					   ws.send(JSON.stringify({UpdateHoldingRegs: data}))
				       });
    
    var input_regs_elems = document.getElementById("input_registers");
    let input_regs = new AreaUpdater(input_regs_elems,
				       function(data) {
					   ws.send(JSON.stringify({UpdateInputRegs: data}))
				       });
    

    ws.onmessage = (msg) => {
        let cmd = JSON.parse(msg.data);
        let holding_registers = cmd.UpdateHoldingRegs;
        if (holding_registers) {
            let a = holding_registers.start;
            for (v of holding_registers.regs) {
                holding_regs.update_value(a, v);
                a++;
            }
        }
	let input_registers = cmd.UpdateInputRegs;
        if (input_registers) {
            let a = input_registers.start;
            for (v of input_registers.regs) {
                input_regs.update_value(a, v);
                a++;
            }
        }

    };
    
    ws.onopen = () => {
        ws.send(JSON.stringify({ RequestHoldingRegs: { start: 0, length: 256 } }))
        ws.send(JSON.stringify({ RequestInputRegs: { start: 0, length: 256 } }))

    };
}
