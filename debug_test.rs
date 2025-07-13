use lexpr::Value;
use serde_lexpr;

fn main() {
    // Test what the server is returning
    let response = "(return 1 \"Hello from Rust!\")";
    println!("Response: {}", response);
    
    let value: Value = lexpr::from_str(response).unwrap();
    println!("Parsed value: {:?}", value);
    
    // Try to deserialize as String
    match serde_lexpr::from_value::<String>(&value) {
        Ok(s) => println!("Successfully deserialized as String: {}", s),
        Err(e) => println!("Error deserializing as String: {}", e),
    }
    
    // Try to extract from the return message
    if let Value::Cons(cons) = &value {
        let items: Vec<Value> = cons.list_iter().map(|v| v.clone()).collect();
        println!("Items: {:?}", items);
        if items.len() == 3 {
            println!("Result value: {:?}", items[2]);
            
            // Try to deserialize the result as String
            match serde_lexpr::from_value::<String>(&items[2]) {
                Ok(s) => println!("Successfully extracted result as String: {}", s),
                Err(e) => println!("Error extracting result as String: {}", e),
            }
        }
    }
}