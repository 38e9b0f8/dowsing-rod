module counter(input logic clk, reset, input logic [7:0] data, output logic [7:0] q);
always_ff @(posedge clk) begin
  if (reset) q <= 0; else q <= data + 1;
end
always_ff @(posedge clk) begin
  if (reset) q <= 0; else q <= data + 1;
end
always_comb begin q = data + 1; end
function automatic int twice(input int value);
  return value * 2;
endfunction
task automatic write_value(input int value);
  $display("value", value);
endtask
endmodule
