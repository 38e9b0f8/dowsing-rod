module counter(input clk, input reset, input [7:0] data, output reg [7:0] q);
always @(posedge clk) begin
  if (reset) q <= 0; else q <= data + 1;
end
always @(posedge clk) begin
  if (reset) q <= 0; else q <= data + 1;
end
function integer twice(input integer value);
  twice = value * 2;
endfunction
task write_value(input integer value);
  $display("value", value);
endtask
endmodule
