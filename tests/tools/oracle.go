package main
import("bufio";"encoding/json";"fmt";"os";b "github.com/BambooEngine/bamboo-core")
type Request struct { Keys string; Method string; Flags uint }
func main(){s:=bufio.NewScanner(os.Stdin);for s.Scan(){var r Request;if json.Unmarshal(s.Bytes(),&r)!=nil{continue};e:=b.NewEngine(b.ParseInputMethod(b.InputMethodDefinitions,r.Method),r.Flags);type State struct {Text string; Raw string; Valid bool};out:=[]State{};for _,c:=range r.Keys{if c=='\b'{e.RemoveLastChar(true)}else{e.ProcessKey(c,b.VietnameseMode)};out=append(out,State{e.GetProcessedString(b.VietnameseMode|b.FullText),e.GetProcessedString(b.EnglishMode|b.FullText),e.IsValid(false)})};v,_:=json.Marshal(out);fmt.Println(string(v))}}
