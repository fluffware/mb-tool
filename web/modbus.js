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
var value_map = {};

function update_value(a, v) {
    let inps = value_map[a];
    if (inps) {
        for (inp of inps) {
            let low = inp.getAttributeNS(MB_NS, "bit_low");
            let high = inp.getAttributeNS(MB_NS, "bit_high");
            if (low != null && high != null) {
                inp.value = (v >> low) & ((1 << (high - low + 1)) - 1);
            } else {
                inp.value = v;
            }
        }
    }
}

function setup() {
    var regs = document.getElementById("holding_registers");
    var values = regs.getElementsByClassName("mb_value");
    for (v of values) {

        let addr = parseInt(v.getAttributeNS(MB_NS, "addr"));
        console.log(addr);
        if (value_map[addr]) {
            value_map[addr].push(v);
        } else {
            value_map[addr] = [v];
        }
        v.addEventListener("change", function(e) {

            ws.send(JSON.stringify({ UpdateHoldingRegs: { start: addr, regs: [parseInt(e.target.value)] } }))
            update_value(addr, e.target.value);
        });
    }
    ws = new WebSocket(socket_uri());
    ws.onmessage = (msg) => {
        let cmd = JSON.parse(msg.data);
        let holding_registers = cmd.UpdateHoldingRegs;
        if (holding_registers) {
            let a = holding_registers.start;
            for (v of holding_registers.regs) {
                update_value(a, v);
                a++;

            }
        }
        console.log(msg)

    };
    ws.onopen = () => {
        ws.send(JSON.stringify({ RequestHoldingRegs: { start: 0, length: 256 } }))

    };
}