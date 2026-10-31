// Queries the Ghidra project of the DLGE executable.
// Args: commands, each "dec:<addr>", "refs:<addr>", "str:<text>", or "fn:<addr>".
// An address below the image base is an offset from the base.
// @category DLGE
import ghidra.app.script.GhidraScript;
import ghidra.app.decompiler.*;
import ghidra.program.model.address.*;
import ghidra.program.model.listing.*;
import ghidra.program.model.symbol.*;
import ghidra.program.model.data.StringDataInstance;

public class DlQuery extends GhidraScript {
    private Address addr(String s) {
        long v = Long.parseUnsignedLong(s.replace("0x", ""), 16);
        long base = currentProgram.getImageBase().getOffset();
        if (v < base) v += base;
        return toAddr(v);
    }

    private String where(Address a) {
        Function f = getFunctionContaining(a);
        return f == null ? "(no function)" : f.getName();
    }

    // Finds the function that holds a, or makes one. The new function starts
    // at the first instruction after the end of the previous function.
    private Function ensureFunction(Address a) throws Exception {
        Function f = getFunctionContaining(a);
        if (f != null) return f;
        Function prev = getFunctionBefore(a);
        Address start = prev == null ? a : prev.getBody().getMaxAddress().next();
        Instruction i = getInstructionAt(start);
        while (i == null && start.compareTo(a) < 0) {
            start = start.add(4);
            i = getInstructionAt(start);
        }
        while (i != null && i.getMnemonicString().startsWith("udf") ) { start = start.add(4); i = getInstructionAt(start); }
        f = createFunction(start, null);
        return f == null ? getFunctionContaining(a) : f;
    }

    // Gives the pointer at a, from its reference if the bytes are not relocated.
    private long target(Address a) throws Exception {
        for (Reference r : getReferencesFrom(a)) return r.getToAddress().getOffset();
        Data d = getDataAt(a);
        if (d != null && d.getValue() instanceof Address) return ((Address) d.getValue()).getOffset();
        return getLong(a);
    }

    @Override
    public void run() throws Exception {
        DecompInterface dec = new DecompInterface();
        dec.openProgram(currentProgram);
        println("BASE " + currentProgram.getImageBase());
        for (String cmd : getScriptArgs()) {
            String[] p = cmd.split(":", 2);
            println("=== " + cmd);
            switch (p[0]) {
                case "dec": {
                    Function f = ensureFunction(addr(p[1]));
                    if (f == null) { println("no function"); break; }
                    println("FUNC " + f.getEntryPoint());
                    DecompileResults r = dec.decompileFunction(f, 120, monitor);
                    println(r.decompileCompleted() ? r.getDecompiledFunction().getC() : "failed: " + r.getErrorMessage());
                    break;
                }
                case "refs": {
                    for (Reference r : getReferencesTo(addr(p[1]))) {
                        println(r.getFromAddress() + " " + r.getReferenceType() + " in " + where(r.getFromAddress()));
                    }
                    break;
                }
                case "fn": {
                    Function f = getFunctionContaining(addr(p[1]));
                    println(f == null ? "none" : f.getName() + " " + f.getBody());
                    break;
                }
                case "str": {
                    DataIterator it = currentProgram.getListing().getDefinedData(true);
                    while (it.hasNext() && !monitor.isCancelled()) {
                        Data d = it.next();
                        if (!d.hasStringValue()) continue;
                        String v = String.valueOf(d.getValue());
                        if (!v.contains(p[1])) continue;
                        println(d.getAddress() + " \"" + v + "\"");
                        for (Reference r : getReferencesTo(d.getAddress())) {
                            println("   <- " + r.getFromAddress() + " in " + where(r.getFromAddress()));
                        }
                    }
                    break;
                }
                case "imm": {
                    // Lists the functions with an instruction that uses one of the values.
                    java.util.Set<Long> want = new java.util.HashSet<>();
                    for (String v : p[1].split(",")) want.add(Long.parseLong(v.replace("0x", ""), 16));
                    java.util.Map<String, Integer> hits = new java.util.TreeMap<>();
                    InstructionIterator it = currentProgram.getListing().getInstructions(true);
                    while (it.hasNext() && !monitor.isCancelled()) {
                        Instruction ins = it.next();
                        for (int o = 0; o < ins.getNumOperands(); o++) {
                            for (Object obj : ins.getOpObjects(o)) {
                                if (!(obj instanceof ghidra.program.model.scalar.Scalar)) continue;
                                ghidra.program.model.scalar.Scalar sc = (ghidra.program.model.scalar.Scalar) obj;
                                long u = sc.getUnsignedValue() & 0xffffffffL;
                                long sg = sc.getSignedValue() & 0xffffffffL;
                                if (want.contains(u) || want.contains(sg)) {
                                    String k = where(ins.getAddress()) + " " + Long.toHexString(u);
                                    hits.merge(k, 1, Integer::sum);
                                }
                            }
                        }
                    }
                    for (var e : hits.entrySet()) println(e.getValue() + " " + e.getKey());
                    break;
                }
                case "callers": {
                    Function f = ensureFunction(addr(p[1]));
                    for (Reference r : getReferencesTo(f.getEntryPoint())) {
                        println(r.getFromAddress() + " " + r.getReferenceType() + " in " + where(r.getFromAddress()));
                    }
                    break;
                }
                case "decat": {
                    // Decompiles a function that starts exactly at the address.
                    Address a = addr(p[1]);
                    Function f = getFunctionAt(a);
                    if (f == null) {
                        Function old = getFunctionContaining(a);
                        if (old != null) removeFunction(old);
                        disassemble(a);
                        f = createFunction(a, null);
                    }
                    if (f == null) { println("no function"); break; }
                    DecompileResults r = dec.decompileFunction(f, 120, monitor);
                    println(r.decompileCompleted() ? r.getDecompiledFunction().getC() : "failed");
                    break;
                }
                case "rtti": {
                    // Reads the class name of a vtable from a pointer slot.
                    for (String v : p[1].split(",")) {
                        try {
                            long vt = target(addr(v));
                            long ti = target(toAddr(vt + 8));
                            long nm = target(toAddr(ti + 8));
                            StringBuilder sb = new StringBuilder();
                            for (int k = 0; k < 120; k++) { byte b = getByte(toAddr(nm + k)); if (b == 0) break; sb.append((char) b); }
                            println("RTTI " + v + " " + sb);
                        } catch (Exception e) { println("RTTI " + v + " ? " + e.getMessage()); }
                    }
                    break;
                }
                case "ptr": {
                    for (String v : p[1].split(",")) {
                        Address a = addr(v);
                        println("PTR " + a + " " + Long.toHexString(getLong(a)));
                    }
                    break;
                }
                case "asm": {
                    // Prints instructions: "asm:<addr>,<count>".
                    String[] q = p[1].split(",");
                    Address a = addr(q[0]);
                    int n = q.length > 1 ? Integer.parseInt(q[1]) : 20;
                    if (getInstructionAt(a) == null) disassemble(a);
                    Instruction ins = getInstructionAt(a);
                    for (int k = 0; k < n && ins != null; k++) {
                        println(ins.getAddress() + "  " + ins);
                        ins = ins.getNext();
                    }
                    break;
                }
                case "jt": {
                    // Prints the targets of a table of 32-bit offsets: "jt:<table>,<count>".
                    String[] q = p[1].split(",");
                    Address t = addr(q[0]);
                    int n = Integer.parseInt(q[1]);
                    for (int k = 0; k < n; k++) {
                        int off = getInt(t.add(4L * k));
                        println(k + " " + t.add(off));
                    }
                    break;
                }
                default: println("unknown command");
            }
        }
        dec.dispose();
    }
}
